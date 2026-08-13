use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, str_prefix::StringLiteralPrefix};
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::fix::edits::fits;
use crate::line_width::LineWidthBuilder;
use crate::rules::odoo::helpers::{dotted_name, odoo_version_applies, wrap_string_literal};
use crate::rules::odoo::settings::OdooVersion;
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
/// and a name can be derived for each one, e.g.
/// `self.env._("%s of %s", count, tier.name)` becomes
/// `self.env._("%(count)s of %(tier_name)s", count=count, tier_name=tier.name)`.
/// Dotted attribute chains join with underscores (`tier.name` becomes `tier_name`), and
/// calls or subscripts borrow the most meaningful identifier inside them
/// (`", ".join(fields_string)` becomes `fields_string`, `values[0]` becomes `values_0`).
///
/// When the rewritten call no longer fits within the configured line length, the fix
/// expands it with one argument per line, wrapping the term itself into a parenthesized
/// implicit string concatenation if needed.
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

/// Byte offsets (right after the `%`) of the positional printf conversions in `text`.
///
/// Only sequences that Python's `%` formatting actually accepts are recognized —
/// `%[flags][width][.precision][length]conversion` — so a stray `%` followed by
/// punctuation (e.g. `"90%. It"`) is plain text, matching runtime behavior. Escaped `%%`
/// and named `%(name)s` conversions are skipped.
fn positional_printf_offsets(text: &str) -> Vec<usize> {
    const CONVERSIONS: &[u8] = b"diouxXeEfFgGcrsa";
    let bytes = text.as_bytes();
    let mut offsets = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        if bytes.get(j) == Some(&b'%') {
            i = j + 1;
            continue;
        }
        let named = bytes.get(j) == Some(&b'(');
        if named {
            let Some(close) = text[j..].find(')') else {
                i = start;
                continue;
            };
            j += close + 1;
        }
        while matches!(bytes.get(j), Some(b'#' | b'0' | b'-' | b' ' | b'+')) {
            j += 1;
        }
        if bytes.get(j) == Some(&b'*') {
            j += 1;
        } else {
            while matches!(bytes.get(j), Some(b'0'..=b'9')) {
                j += 1;
            }
        }
        if bytes.get(j) == Some(&b'.') {
            j += 1;
            if bytes.get(j) == Some(&b'*') {
                j += 1;
            } else {
                while matches!(bytes.get(j), Some(b'0'..=b'9')) {
                    j += 1;
                }
            }
        }
        if matches!(bytes.get(j), Some(b'h' | b'l' | b'L')) {
            j += 1;
        }
        if bytes.get(j).is_some_and(|byte| CONVERSIONS.contains(byte)) {
            if !named {
                offsets.push(start);
            }
            i = j + 1;
        } else {
            i = start;
        }
    }
    offsets
}

/// Counts positional printf conversions (`%s`, `%d`, ... but not `%%` or `%(name)s`).
fn count_positional_printf(text: &str) -> usize {
    positional_printf_offsets(text).len()
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

    if checker.is_rule_enabled(Rule::TranslationContainsVariable)
        // Interpolating before translation was only flagged up to Odoo 13.0; from 14.0 on,
        // `translation-not-lazy`-family checks handle this instead.
        && odoo_version_applies(checker, None, Some(OdooVersion::new(13, 0)))
    {
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

/// Derives a placeholder name for an interpolated argument.
///
/// A name or dotted attribute chain names the placeholder directly, with dots turned into
/// underscores (`tier.campaign_id.name` becomes `tier_campaign_id_name`). Expressions
/// without a direct name borrow the most meaningful identifier inside them:
///
/// - a call is named after its first nameable positional argument, then a string-key
///   argument, then the callee itself: `", ".join(fields_string)` becomes
///   `fields_string`, `oracle_leave.get("startDateTime")` becomes `startDateTime`, and
///   `sale.mapped("name")` becomes `sale_mapped`. Arguments rooted at `self`/`cls` only
///   win when nothing else is nameable, so helpers taking the environment first, like
///   `format_date(self.env, record.start_date)`, are named after the value
///   (`record_start_date`) instead of `self_env`;
/// - a subscript is named after its base plus a literal or nameable index, and an
///   attribute access chains onto whatever its receiver derives: `values[0]` becomes
///   `values_0`, `vals["name"]` becomes `vals_name`, and `closures[0].name` becomes
///   `closures_0_name`;
/// - operators carry over the operand holding the value (`coverage * 100` becomes
///   `coverage`), an f-string its first nameable interpolation, a conditional expression
///   its branches (`_(" In: %s.", title) if title else ""` becomes `title`), and a
///   comprehension its element, falling back to the iterated source
///   (`["%s: %s" % (p.ref, p.name) for p in partners]` becomes `partners`).
///
/// Anything without an identifier anywhere inside (plain literals, ...) has no obvious
/// name, so the caller skips the fix for the whole call.
fn placeholder_name(expr: &Expr) -> Option<String> {
    if let Some(dotted) = dotted_name(expr) {
        return Some(dotted.replace('.', "_"));
    }
    match expr {
        Expr::Call(call) => {
            let arg_names: Vec<String> = call
                .arguments
                .args
                .iter()
                .filter_map(placeholder_name)
                .collect();
            arg_names
                .iter()
                .find(|name| !is_self_rooted(name))
                .or_else(|| arg_names.first())
                .cloned()
                // `.get("startDateTime")`-style: a string key argument works as a name.
                .or_else(|| {
                    call.arguments.args.iter().find_map(|arg| match arg {
                        Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                            let key = value.to_str();
                            ruff_python_stdlib::identifiers::is_identifier(key)
                                .then(|| key.to_string())
                        }
                        _ => None,
                    })
                })
                .or_else(|| placeholder_name(&call.func))
        }
        // `refundable_closures[0].name` and other attribute accesses whose receiver isn't a
        // plain name chain (those are already covered by `dotted_name` above).
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            Some(format!("{}_{attr}", placeholder_name(value)?))
        }
        // `coverage * 100`, `-amount`: name the operand carrying the value.
        Expr::BinOp(ast::ExprBinOp { left, right, .. }) => {
            placeholder_name(left).or_else(|| placeholder_name(right))
        }
        Expr::UnaryOp(ast::ExprUnaryOp { operand, .. }) => placeholder_name(operand),
        // `f"{entry_amount:,.2f}"`: name the first nameable interpolated expression.
        Expr::FString(fstring) => fstring
            .value
            .elements()
            .filter_map(ast::InterpolatedStringElement::as_interpolation)
            .find_map(|interpolation| placeholder_name(&interpolation.expression)),
        Expr::Subscript(subscript) => {
            let base = placeholder_name(&subscript.value)?;
            let index = match subscript.slice.as_ref() {
                Expr::NumberLiteral(ast::ExprNumberLiteral {
                    value: ast::Number::Int(int),
                    ..
                }) => Some(int.to_string()),
                Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                    let key = value.to_str();
                    ruff_python_stdlib::identifiers::is_identifier(key).then(|| key.to_string())
                }
                index => placeholder_name(index),
            };
            Some(match index {
                Some(index) => format!("{base}_{index}"),
                None => base,
            })
        }
        Expr::If(if_exp) => {
            placeholder_name(&if_exp.body).or_else(|| placeholder_name(&if_exp.orelse))
        }
        Expr::ListComp(comp) => comprehension_name(&comp.elt, &comp.generators),
        Expr::SetComp(comp) => comprehension_name(&comp.elt, &comp.generators),
        Expr::Generator(comp) => comprehension_name(&comp.elt, &comp.generators),
        _ => None,
    }
}

/// Names a comprehension after its element, falling back to the source being iterated.
fn comprehension_name(elt: &Expr, generators: &[ast::Comprehension]) -> Option<String> {
    placeholder_name(elt).or_else(|| {
        generators
            .iter()
            .find_map(|generator| placeholder_name(&generator.iter))
    })
}

/// Returns `true` for derived names rooted at `self` or `cls` (e.g. `self`,
/// `self_env_company`), which rarely describe the interpolated value itself.
fn is_self_rooted(name: &str) -> bool {
    name == "self" || name == "cls" || name.starts_with("self_") || name.starts_with("cls_")
}

/// Builds a fix rewriting `_("%s of %s", count, tier.name)` into
/// `_("%(count)s of %(tier_name)s", count=count, tier_name=tier.name)`.
///
/// Each placeholder is named after the argument it interpolates (see [`placeholder_name`]).
/// The values are passed as keyword arguments because Odoo's translation helpers format
/// with `translation % (args or kwargs)` — a dict passed positionally would arrive wrapped
/// in a tuple and fail to format named placeholders.
///
/// No fix is offered when the term or the arguments can't be converted faithfully: values
/// interpolated outside the call (`_("%s %s") % (a, b)`), arguments without a derivable
/// name, an argument count that doesn't match the placeholder count, or a term already
/// mixing in named/`{}` placeholders.
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
    let offsets = positional_printf_offsets(text);
    if args.is_empty() || args.len() != offsets.len() {
        return None;
    }

    // Derive a placeholder name from each argument. The same expression interpolated twice
    // can share a single keyword argument; distinct expressions colliding on a name (e.g.
    // `a.b` and `a_b`) can't be merged, so no fix is offered.
    let locator = checker.locator();
    let mut names = Vec::with_capacity(args.len());
    let mut keywords: Vec<(String, &str)> = Vec::with_capacity(args.len());
    for arg in args {
        let name = placeholder_name(arg)?;
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

    // Rebuild the term with `(name)` spliced into each positional conversion, keeping the
    // conversion spec: `%s .. %.2f` becomes `%(count)s .. %(total).2f`.
    let mut new_text = String::with_capacity(text.len() + names.len() * 8);
    let mut last = 0;
    for (offset, name) in offsets.iter().zip(&names) {
        new_text.push_str(&text[last..*offset]);
        new_text.push('(');
        new_text.push_str(name);
        new_text.push(')');
        last = *offset;
    }
    new_text.push_str(&text[last..]);

    let flags = checker.default_string_flags().with_prefix({
        if value.is_unicode() {
            StringLiteralPrefix::Unicode
        } else {
            StringLiteralPrefix::Empty
        }
    });
    let term_repr = checker.generator().expr(
        &ast::StringLiteral {
            value: new_text.clone().into_boxed_str(),
            flags,
            range: TextRange::default(),
            node_index: ruff_python_ast::AtomicNodeIndex::NONE,
        }
        .into(),
    );
    let replacement = format!(
        "{term_repr}, {}",
        keywords
            .iter()
            .map(|(name, source)| format!("{name}={source}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Prefer the in-place rewrite when the resulting line still fits, measuring with the
    // rest of the line that follows the replaced range (closing parens, commas, ...).
    let replaced_range = TextRange::new(term.start(), args.last()?.end());
    let suffix = locator.slice(TextRange::new(
        replaced_range.end(),
        locator.line_end(replaced_range.end()),
    ));
    if fits(
        &format!("{replacement}{suffix}"),
        term.into(),
        locator,
        checker.settings().pycodestyle.max_line_length,
        checker.settings().tab_size,
    ) {
        return Some(Fix::unsafe_edit(Edit::range_replacement(
            replacement,
            replaced_range,
        )));
    }

    // The one-line rewrite exceeds the line length: expand the translation call instead,
    // one argument per line, wrapping the term itself into a parenthesized implicit
    // concatenation when it doesn't fit on its own line either. The trailing comma keeps
    // formatters from collapsing the call back into one line.
    let max_line_length = checker.settings().pycodestyle.max_line_length.value() as usize;
    let line_start = locator.line_start(call.start());
    let indent: String = locator
        .slice(TextRange::new(line_start, call.start()))
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let inner_indent = format!("{indent}{}", checker.stylist().indentation().as_str());
    let line_ending = checker.stylist().line_ending().as_str();

    let term_width = LineWidthBuilder::new(checker.settings().tab_size)
        .add_str(&inner_indent)
        .add_str(&term_repr)
        .get();
    // `< max_line_length` (not `<=`): the trailing comma takes one more column. The
    // wrapped pieces reserve that column too, since the last piece also carries the comma.
    let term_pieces = if term_width < max_line_length {
        vec![term_repr]
    } else {
        wrap_string_literal(
            checker,
            flags,
            &new_text,
            &inner_indent,
            max_line_length.saturating_sub(1),
        )
    };

    let mut expanded = String::from("(");
    for piece in &term_pieces {
        expanded.push_str(line_ending);
        expanded.push_str(&inner_indent);
        expanded.push_str(piece);
    }
    expanded.push(',');
    for (name, source) in &keywords {
        expanded.push_str(line_ending);
        expanded.push_str(&inner_indent);
        expanded.push_str(name);
        expanded.push('=');
        expanded.push_str(source);
        expanded.push(',');
    }
    expanded.push_str(line_ending);
    expanded.push_str(&indent);
    expanded.push(')');
    Some(Fix::unsafe_edit(Edit::range_replacement(
        expanded,
        call.arguments.range(),
    )))
}
