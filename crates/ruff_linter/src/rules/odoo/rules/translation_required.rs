use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for user-facing string literals that reach the user without a translation call:
/// the `body`/`subject` of a `message_post`, and the message of a raised Odoo exception.
///
/// ## Why is this bad?
/// Chatter text and error messages are shown to users; a plain literal is displayed as-is
/// for every language instead of being looked up in the translations.
///
/// ## Example
/// ```python
/// self.message_post(body="Order confirmed")
/// raise UserError("Order cannot be confirmed")
/// ```
///
/// Use instead:
/// ```python
/// self.message_post(body=self.env._("Order confirmed"))
/// raise UserError(self.env._("Order cannot be confirmed"))
/// ```
///
/// Interpolated literals (`"..." % values`, `"...".format(values)`) are flagged too: the
/// term must be translated before the values are interpolated.
///
/// The suggested translation call follows the configured `odoo-version`: `self.env._` from
/// Odoo 18.0 on (and when no version is configured), the bare `_` before that. This keeps
/// the suggestion consistent with `prefer-env-translation` (`ODOO024`), which only applies
/// from 18.0.
///
/// Files under a `tests/` directory are ignored, matching pylint-odoo.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.5")]
pub(crate) struct TranslationRequired {
    /// The called function the literal was passed to, e.g. `message_post` or `UserError`.
    func: String,
    /// The keyword the literal was passed as, including the `=`, or empty when positional.
    keyword: String,
    /// The translation call to suggest, e.g. `self.env._`.
    translation_method: &'static str,
}

impl Violation for TranslationRequired {
    #[derive_message_formats]
    fn message(&self) -> String {
        let TranslationRequired {
            func,
            keyword,
            translation_method,
        } = self;
        format!(
            r#"String parameter on "{func}" requires translation. Use {keyword}{translation_method}(...)"#
        )
    }
}

/// Odoo exception classes whose first argument is the user-facing message, mirroring
/// pylint-odoo's `DFTL_ODOO_EXCEPTIONS`. These are matched by name only: the exceptions are
/// usually imported (`from odoo.exceptions import UserError`) or reached through the module
/// (`odoo.exceptions.UserError`), and both spellings end in the same identifier.
const ODOO_EXCEPTIONS: &[&str] = &[
    "AccessDenied",
    "AccessError",
    "CacheMiss",
    "except_orm",
    "MissingError",
    "RedirectWarning",
    "UserError",
    "ValidationError",
    "Warning",
];

/// The translation call to recommend for the configured Odoo version.
///
/// `self.env._` replaced the bare `_` in Odoo 18.0. Recommending the bare `_` on a 18.0+
/// codebase would immediately be flagged by `prefer-env-translation` (`ODOO024`), so the two
/// rules have to agree on the cutoff.
fn translation_method(checker: &Checker) -> &'static str {
    if odoo_version_applies(checker, Some(OdooVersion::new(18, 0)), None) {
        "self.env._"
    } else {
        "_"
    }
}

/// Returns the literal being passed untranslated: the string itself, the left side of a
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

/// Chatter text and exception messages raised from tests are not user-facing; matching
/// pylint-odoo, skip files whose immediate directory is `tests`.
fn in_tests_dir(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("tests")
}

/// ODOO049
pub(crate) fn translation_required(checker: &Checker, call: &ast::ExprCall, path: &Path) {
    if in_tests_dir(path) {
        return;
    }
    let Expr::Attribute(ast::ExprAttribute { attr, .. }) = call.func.as_ref() else {
        return;
    };
    if attr != "message_post" {
        return;
    }
    let translation_method = translation_method(checker);

    for arg in &call.arguments.args {
        if let Some(literal) = untranslated_literal(arg) {
            checker.report_diagnostic(
                TranslationRequired {
                    func: "message_post".to_string(),
                    keyword: String::new(),
                    translation_method,
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
                    func: "message_post".to_string(),
                    keyword: format!("{name}="),
                    translation_method,
                },
                literal.range(),
            );
        }
    }
}

/// ODOO049
///
/// The `raise UserError("...")` half of the check. Only the first positional argument is
/// inspected, because for every Odoo exception in [`ODOO_EXCEPTIONS`] that argument is the
/// message shown to the user; pylint-odoo looks at the same single argument.
pub(crate) fn translation_required_raise(checker: &Checker, raise: &ast::StmtRaise, path: &Path) {
    if in_tests_dir(path) {
        return;
    }
    // A bare `raise` (re-raising the active exception) has no message to translate.
    let Some(Expr::Call(call)) = raise.exc.as_deref() else {
        return;
    };
    let func = match call.func.as_ref() {
        Expr::Name(ast::ExprName { id, .. }) => id.as_str(),
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str(),
        _ => return,
    };
    if !ODOO_EXCEPTIONS.contains(&func) {
        return;
    }
    let Some(message) = call.arguments.args.first() else {
        return;
    };
    let Some(literal) = untranslated_literal(message) else {
        return;
    };
    checker.report_diagnostic(
        TranslationRequired {
            func: func.to_string(),
            keyword: String::new(),
            translation_method: translation_method(checker),
        },
        literal.range(),
    );
}
