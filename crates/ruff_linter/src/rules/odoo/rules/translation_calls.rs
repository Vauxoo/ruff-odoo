use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, str_prefix::StringLiteralPrefix};
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::rules::odoo::helpers::dotted_name;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for translation calls whose term already has variables interpolated into it,
/// e.g. `_("Hello %s" % name)` or `_("Hello {}".format(name))`.
///
/// ## Why is this bad?
/// The interpolation happens *before* translation, so the looked-up term contains the
/// runtime value and never matches the exported translation entry.
///
/// ## Example
/// ```python
/// _("Hello %s" % name)
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s") % name
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct TranslationContainsVariable;

impl Violation for TranslationContainsVariable {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Translatable term contains variables. Interpolate outside the translation call".to_string()
    }
}

/// ## What it does
/// Checks for translated format strings with two or more positional placeholders
/// (`_("%s %s")` or `_("{} {}")`).
///
/// ## Why is this bad?
/// Translators can't reorder positional placeholders, but many languages need a different
/// word order. Named placeholders (`%(name)s`, `{name}`) keep the translation reorderable.
///
/// ## Example
/// ```python
/// _("%s of %s") % (count, total)
/// ```
///
/// Use instead:
/// ```python
/// _("%(count)s of %(total)s") % {"count": count, "total": total}
/// ```
///
/// ## Fix safety
/// A fix is offered when the values are passed as arguments of the translation call itself
/// and each one is a plain name or dotted attribute chain, e.g.
/// `self.env._("%s of %s", count, tier.name)` becomes
/// `self.env._("%(count)s of %(tier_name)s", count=count, tier_name=tier.name)`.
///
/// The fix is marked unsafe because it changes the source translation term: existing
/// translations keyed on the old term (in `.po` files) no longer match and must be
/// re-exported and re-translated.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct TranslationPositional;

impl Violation for TranslationPositional {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Translation method is using positional string printf formatting with multiple \
         arguments. Use named placeholders instead"
            .to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Convert to named placeholders passed as keyword arguments".to_string())
    }
}

/// ## What it does
/// Checks for `.format(...)` called on the *result* of a translation, e.g.
/// `_("...{}...").format(value)`.
///
/// ## Why is this bad?
/// Calling `str.format` on translated text lets a malicious translation access attributes
/// of the format arguments (`{0.__class__}`-style injection).
///
/// ## Example
/// ```python
/// _("Hello {}").format(name)
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s") % name
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct TranslationInjection;

impl Violation for TranslationInjection {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Do not use str.format on translation methods. Use placeholders instead".to_string()
    }
}

/// Returns `true` if `func` is a translation function reference: the bare `_`/`_lt` names,
/// or an attribute path ending in them (e.g. `self.env._`).
fn is_translation_func(func: &Expr) -> bool {
    match func {
        Expr::Name(ast::ExprName { id, .. }) => id == "_" || id == "_lt",
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr == "_" || attr == "_lt",
        _ => false,
    }
}

/// Counts positional printf specifiers (`%s`, `%d`, ... but not `%%` or `%(name)s`) in `text`.
fn count_positional_printf(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            match bytes.get(i + 1) {
                Some(b'%') => i += 1, // escaped %%
                Some(b'(') => {
                    // named placeholder %(name)s — skip past it
                    if let Some(close) = text[i..].find(')') {
                        i += close;
                    }
                }
                Some(_) => count += 1,
                None => {}
            }
        }
        i += 1;
    }
    count
}

/// Counts positional `str.format` fields (`{}` or `{0}`, but not `{name}` or `{{`).
fn count_positional_format(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '{' {
            continue;
        }
        if matches!(chars.peek(), Some((_, '{'))) {
            chars.next(); // escaped {{
            continue;
        }
        let field: String = text[i + 1..].chars().take_while(|&c| c != '}').collect();
        let name = field.split([':', '!', '.', '[']).next().unwrap_or("");
        if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
            count += 1;
        }
    }
    count
}

/// ODOO041, ODOO042, ODOO043
pub(crate) fn translation_calls(checker: &Checker, call: &ast::ExprCall) {
    // ODOO043: `_(...).format(...)` — format called on the translated result.
    if checker.is_rule_enabled(Rule::TranslationInjection)
        && let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref()
        && attr == "format"
        && matches!(value.as_ref(), Expr::Call(inner) if is_translation_func(&inner.func))
    {
        checker.report_diagnostic(TranslationInjection, call.range());
    }

    if !is_translation_func(&call.func) {
        return;
    }
    let Some(first_arg) = call.arguments.args.first() else {
        return;
    };

    if checker.is_rule_enabled(Rule::TranslationContainsVariable) {
        let interpolated = match first_arg {
            // _('...' % variables)
            Expr::BinOp(ast::ExprBinOp {
                op: ast::Operator::Mod,
                left,
                ..
            }) => matches!(left.as_ref(), Expr::StringLiteral(_)),
            // _('...'.format(variables))
            Expr::Call(inner) => matches!(
                inner.func.as_ref(),
                Expr::Attribute(ast::ExprAttribute { value, attr, .. })
                    if attr == "format" && matches!(value.as_ref(), Expr::StringLiteral(_))
            ),
            _ => false,
        };
        if interpolated {
            checker.report_diagnostic(TranslationContainsVariable, call.range());
        }
    }

    if checker.is_rule_enabled(Rule::TranslationPositional)
        && let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = first_arg
    {
        let text = value.to_str();
        if count_positional_printf(text) >= 2 || count_positional_format(text) >= 2 {
            let mut diagnostic = checker.report_diagnostic(TranslationPositional, call.range());
            if let Some(fix) = convert_to_named_placeholders(checker, call, value, text) {
                diagnostic.set_fix(fix);
            }
        }
    }
}

/// Builds a fix rewriting `_("%s of %s", count, tier.name)` into
/// `_("%(count)s of %(tier_name)s", count=count, tier_name=tier.name)`.
///
/// Each placeholder is named after the argument it interpolates, with dots turned into
/// underscores (`tier.campaign_id.name` becomes `tier_campaign_id_name`). The values are
/// passed as keyword arguments because Odoo's translation helpers format with
/// `translation % (args or kwargs)` — a dict passed positionally would arrive wrapped in a
/// tuple and fail to format named placeholders.
///
/// No fix is offered when the term or the arguments can't be converted faithfully: values
/// interpolated outside the call (`_("%s %s") % (a, b)`), arguments without an obvious name
/// (calls, literals, subscripts), an argument count that doesn't match the placeholder
/// count, or a term already mixing in named/`{}` placeholders.
fn convert_to_named_placeholders(
    checker: &Checker,
    call: &ast::ExprCall,
    value: &ast::StringLiteralValue,
    text: &str,
) -> Option<Fix> {
    // Odoo's translation helpers only apply printf-style formatting, so a `{}`-style term
    // with in-call arguments is broken beyond what this fix can repair.
    if count_positional_format(text) >= 2 {
        return None;
    }
    // A term mixing named and positional placeholders is already broken; don't touch it.
    if text.contains("%(") {
        return None;
    }
    if !call.arguments.keywords.is_empty() {
        return None;
    }
    let (term, args) = call.arguments.args.split_first()?;
    if args.is_empty() || args.len() != count_positional_printf(text) {
        return None;
    }

    // Derive a placeholder name from each argument. The same expression interpolated twice
    // can share a single keyword argument; distinct expressions colliding on a name (e.g.
    // `a.b` and `a_b`) can't be merged, so no fix is offered.
    let locator = checker.locator();
    let mut names = Vec::with_capacity(args.len());
    let mut keywords: Vec<(String, &str)> = Vec::with_capacity(args.len());
    for arg in args {
        let name = dotted_name(arg)?.replace('.', "_");
        // `_lt` (`LazyTranslate`) consumes `_module`/`_default_lang` keywords itself.
        if matches!(name.as_str(), "_module" | "_default_lang") {
            return None;
        }
        let source = locator.slice(arg.range());
        match keywords.iter().find(|(existing, _)| *existing == name) {
            Some((_, existing_source)) if *existing_source == source => {}
            Some(_) => return None,
            None => keywords.push((name.clone(), source)),
        }
        names.push(name);
    }

    // Rebuild the term with `(name)` spliced into each positional placeholder, keeping the
    // conversion spec: `%s .. %.2f` becomes `%(count)s .. %(total).2f`.
    let mut new_text = String::with_capacity(text.len() + names.len() * 8);
    let mut names_iter = names.iter();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        new_text.push(c);
        if c != '%' {
            continue;
        }
        match chars.peek() {
            Some('%') => {
                new_text.push('%');
                chars.next();
            }
            // A trailing lone `%` can't be formatted at runtime; leave the call alone.
            None => return None,
            Some(_) => {
                let name = names_iter.next()?;
                new_text.push('(');
                new_text.push_str(name);
                new_text.push(')');
            }
        }
    }

    let node = ast::StringLiteral {
        value: new_text.into_boxed_str(),
        flags: checker.default_string_flags().with_prefix({
            if value.is_unicode() {
                StringLiteralPrefix::Unicode
            } else {
                StringLiteralPrefix::Empty
            }
        }),
        range: TextRange::default(),
        node_index: ruff_python_ast::AtomicNodeIndex::NONE,
    };
    let keywords = keywords
        .iter()
        .map(|(name, source)| format!("{name}={source}"))
        .collect::<Vec<_>>()
        .join(", ");
    let replacement = format!("{}, {keywords}", checker.generator().expr(&node.into()));
    Some(Fix::unsafe_edit(Edit::range_replacement(
        replacement,
        TextRange::new(term.start(), args.last()?.end()),
    )))
}
