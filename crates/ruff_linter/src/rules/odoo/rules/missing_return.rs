use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_body;
use ruff_python_ast::{self as ast, ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks that a method calling `super()` also has a `return` statement.
///
/// ## Why is this bad?
/// A method that calls `super()` but never returns its result (or any other value) usually
/// means the return value of the base implementation is silently dropped.
///
/// ## Example
/// ```python
/// def write(self, vals):
///     super().write(vals)
/// ```
///
/// Use instead:
/// ```python
/// def write(self, vals):
///     return super().write(vals)
/// ```
///
/// ## Options
/// - `lint.odoo.no-missing-return`
///
/// Names the methods exempt from returning, not the ones checked.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct MissingReturn {
    name: String,
}

impl Violation for MissingReturn {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingReturn { name } = self;
        format!("Missing `return` (`super` is used) in method {name}")
    }
}

const NO_MISSING_RETURN: &[&str] = &[
    "__init__",
    "_register_hook",
    "setUp",
    "setUpClass",
    "tearDown",
    "tearDownClass",
];

/// Returns `true` if a `return` statement appears directly in `body`, recursing through
/// control-flow blocks but not into nested function/class definitions.
fn contains_return(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::Return(_) => true,
        Stmt::If(ast::StmtIf {
            body,
            elif_else_clauses,
            ..
        }) => {
            contains_return(body)
                || elif_else_clauses
                    .iter()
                    .any(|clause| contains_return(&clause.body))
        }
        Stmt::For(ast::StmtFor { body, orelse, .. })
        | Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
            contains_return(body) || contains_return(orelse)
        }
        Stmt::With(ast::StmtWith { body, .. }) => contains_return(body),
        Stmt::Try(ast::StmtTry {
            body,
            handlers,
            orelse,
            finalbody,
            ..
        }) => {
            contains_return(body)
                || handlers.iter().any(|handler| {
                    let ExceptHandler::ExceptHandler(handler) = handler;
                    contains_return(&handler.body)
                })
                || contains_return(orelse)
                || contains_return(finalbody)
        }
        Stmt::Match(ast::StmtMatch { cases, .. }) => {
            cases.iter().any(|case| contains_return(&case.body))
        }
        _ => false,
    })
}

/// ODW8110
pub(crate) fn missing_return(checker: &Checker, function_def: &ast::StmtFunctionDef) {
    if !checker.semantic().current_scope().kind.is_class() {
        return;
    }
    if checker
        .settings()
        .odoo
        .no_missing_return
        .contains(function_def.name.as_str(), NO_MISSING_RETURN)
    {
        return;
    }

    let calls_super = any_over_body(&function_def.body, |expr| {
        matches!(
            expr,
            Expr::Call(ast::ExprCall { func, .. })
                if matches!(func.as_ref(), Expr::Name(name) if name.id == "super")
        )
    });
    if !calls_super {
        return;
    }

    let is_generator = any_over_body(&function_def.body, |expr| {
        matches!(expr, Expr::Yield(_) | Expr::YieldFrom(_))
    });
    if is_generator {
        return;
    }

    if !contains_return(&function_def.body) {
        checker.report_diagnostic(
            MissingReturn {
                name: function_def.name.to_string(),
            },
            function_def.name.range(),
        );
    }
}
