use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, ExceptHandler, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for `except` blocks whose entire body is a bare `pass`.
///
/// ## Why is this bad?
/// Silently swallowing an exception without at least logging it makes bugs
/// hard to diagnose. If the exception is genuinely expected and ignorable,
/// prefer `contextlib.suppress`, which documents that intent; otherwise, log
/// the exception.
///
/// ## Example
/// ```python
/// try:
///     do_something()
/// except Exception:
///     pass
/// ```
///
/// Use instead:
/// ```python
/// try:
///     do_something()
/// except Exception:
///     _logger.exception("do_something failed")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct ExceptPass;

impl Violation for ExceptPass {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`except` block with only a `pass` statement; consider logging the exception".to_string()
    }
}

/// ODW8138
pub(crate) fn except_pass(checker: &Checker, handlers: &[ExceptHandler]) {
    for handler in handlers {
        let ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler { name, body, .. }) =
            handler;
        if name.is_none() && matches!(body.as_slice(), [Stmt::Pass(_)]) {
            checker.report_diagnostic(ExceptPass, handler.range());
        }
    }
}
