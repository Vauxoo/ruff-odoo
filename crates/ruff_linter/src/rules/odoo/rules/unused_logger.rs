use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_body;
use ruff_python_ast::{self as ast, Expr, Stmt, Suite};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::fix::edits::delete_stmt;
use crate::{AlwaysFixableViolation, Fix};

/// ## What it does
/// Checks for a module-level `_logger = logging.getLogger(__name__)` that is never used.
///
/// ## Why is this bad?
/// An unused logger is dead code; it should either be used to log something, or removed.
///
/// ## Example
/// ```python
/// import logging
///
/// _logger = logging.getLogger(__name__)
/// ```
///
/// Use instead:
/// ```python
/// import logging
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct UnusedLogger;

impl AlwaysFixableViolation for UnusedLogger {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Unused `_logger`".to_string()
    }

    fn fix_title(&self) -> String {
        "Remove unused `_logger`".to_string()
    }
}

/// Returns `true` if `expr` is a call to `logging.getLogger(__name__)`.
fn is_get_logger_dunder_name_call(expr: &Expr) -> bool {
    let Expr::Call(ast::ExprCall {
        func, arguments, ..
    }) = expr
    else {
        return false;
    };
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() else {
        return false;
    };
    if !matches!(value.as_ref(), Expr::Name(name) if name.id == "logging") || attr != "getLogger" {
        return false;
    }
    let [Expr::Name(name)] = arguments.args.as_ref() else {
        return false;
    };
    name.id == "__name__"
}

/// Returns `true` if `expr` is an attribute access on a `_logger` name (e.g. `_logger.info(...)`).
fn is_logger_usage(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Attribute(ast::ExprAttribute { value, .. })
            if matches!(value.as_ref(), Expr::Name(name) if name.id == "_logger")
    )
}

/// ODOO006
pub(crate) fn unused_logger(checker: &Checker, suite: &Suite) {
    let Some(assign_stmt) = suite.iter().find(|stmt| {
        let Stmt::Assign(assign) = stmt else {
            return false;
        };
        let [Expr::Name(target)] = assign.targets.as_slice() else {
            return false;
        };
        target.id == "_logger" && is_get_logger_dunder_name_call(&assign.value)
    }) else {
        return;
    };

    if any_over_body(suite, is_logger_usage) {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(UnusedLogger, assign_stmt.range());
    diagnostic.set_fix(Fix::safe_edit(delete_stmt(
        assign_stmt,
        None,
        checker.locator(),
        checker.indexer(),
    )));
}
