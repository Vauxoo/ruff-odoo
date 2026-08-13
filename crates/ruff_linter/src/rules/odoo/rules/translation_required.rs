use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for user-facing string literals passed to `message_post` (as `body` or
/// `subject`) without a translation call.
///
/// ## Why is this bad?
/// Text posted to the chatter is shown to users; a plain literal is displayed as-is for
/// every language instead of being looked up in the translations.
///
/// ## Example
/// ```python
/// self.message_post(body="Order confirmed")
/// ```
///
/// Use instead:
/// ```python
/// self.message_post(body=_("Order confirmed"))
/// ```
///
/// Interpolated literals (`"..." % values`, `"...".format(values)`) are flagged too: the
/// term must be translated before the values are interpolated.
///
/// Files under a `tests/` directory are ignored, matching pylint-odoo.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct TranslationRequired {
    keyword: String,
}

impl Violation for TranslationRequired {
    #[derive_message_formats]
    fn message(&self) -> String {
        let TranslationRequired { keyword } = self;
        format!(r#"String parameter on "message_post" requires translation. Use {keyword}_(...)"#)
    }
}

/// Returns the literal being posted untranslated: the string itself, the left side of a
/// `"..." % values` interpolation, or the receiver of a `"...".format(values)` call.
fn untranslated_literal(value: &Expr) -> Option<&Expr> {
    match value {
        Expr::StringLiteral(_) | Expr::FString(_) => Some(value),
        // "String %s" % values — the right side is translatable itself only when it's a
        // call (or a tuple/list of calls), e.g. body=_("...") % values.
        Expr::BinOp(ast::ExprBinOp {
            op: ast::Operator::Mod,
            left,
            right,
            ..
        }) if matches!(left.as_ref(), Expr::StringLiteral(_) | Expr::FString(_)) => {
            let translatable = match right.as_ref() {
                Expr::Call(_) => true,
                Expr::Tuple(ast::ExprTuple { elts, .. })
                | Expr::List(ast::ExprList { elts, .. }) => {
                    elts.iter().all(|elt| matches!(elt, Expr::Call(_)))
                }
                _ => false,
            };
            (!translatable).then_some(left.as_ref())
        }
        // "String {}".format(values)
        Expr::Call(ast::ExprCall { func, .. }) => match func.as_ref() {
            Expr::Attribute(ast::ExprAttribute {
                value: receiver,
                attr,
                ..
            }) if attr == "format"
                && matches!(receiver.as_ref(), Expr::StringLiteral(_) | Expr::FString(_)) =>
            {
                Some(receiver.as_ref())
            }
            _ => None,
        },
        _ => None,
    }
}

/// ODOO049
pub(crate) fn translation_required(checker: &Checker, call: &ast::ExprCall, path: &Path) {
    // Chatter text in tests is not user-facing; matching pylint-odoo, skip files whose
    // immediate directory is `tests`.
    if path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("tests")
    {
        return;
    }
    let Expr::Attribute(ast::ExprAttribute { attr, .. }) = call.func.as_ref() else {
        return;
    };
    if attr != "message_post" {
        return;
    }

    for arg in &call.arguments.args {
        if let Some(literal) = untranslated_literal(arg) {
            checker.report_diagnostic(
                TranslationRequired {
                    keyword: String::new(),
                },
                literal.range(),
            );
        }
    }
    for keyword in &call.arguments.keywords {
        let Some(name) = keyword.arg.as_deref() else {
            continue;
        };
        if !matches!(name, "subject" | "body") {
            continue;
        }
        if let Some(literal) = untranslated_literal(&keyword.value) {
            checker.report_diagnostic(
                TranslationRequired {
                    keyword: format!("{name}="),
                },
                literal.range(),
            );
        }
    }
}
