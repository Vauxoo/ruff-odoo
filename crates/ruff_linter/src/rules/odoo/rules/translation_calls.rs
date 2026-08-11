use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Rule;

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
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct TranslationPositional;

impl Violation for TranslationPositional {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Translation method is using positional string printf formatting with multiple \
         arguments. Use named placeholders instead"
            .to_string()
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
            checker.report_diagnostic(TranslationPositional, call.range());
        }
    }
}
