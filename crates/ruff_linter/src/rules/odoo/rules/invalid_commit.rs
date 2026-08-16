use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{CURSOR_EXPRS, dotted_name};

/// ## What it does
/// Checks for direct calls to `cr.commit()` (or `self.cr.commit()`, `self._cr.commit()`,
/// `self.env.cr.commit()`).
///
/// ## Why is this bad?
/// Committing the transaction directly bypasses Odoo's own transaction management and can
/// leave the database in an inconsistent state if a later step fails.
///
/// ## Example
/// ```python
/// self.env.cr.commit()
/// ```
///
/// ## Options
/// - `lint.odoo.cursor-expr`
///
/// Shared with `sql-injection` (`ODE8103`): both ask whether an expression is a cursor.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct InvalidCommit;

impl Violation for InvalidCommit {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Use of cr.commit() directly".to_string()
    }
}

/// ODE8102
pub(crate) fn invalid_commit(checker: &Checker, call: &ast::ExprCall) {
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
        return;
    };
    if attr != "commit" {
        return;
    }
    let Some(cursor) = dotted_name(value) else {
        return;
    };
    if checker
        .settings()
        .odoo
        .cursor_expr
        .contains(cursor.as_str(), CURSOR_EXPRS)
    {
        checker.report_diagnostic(InvalidCommit, call.range());
    }
}
