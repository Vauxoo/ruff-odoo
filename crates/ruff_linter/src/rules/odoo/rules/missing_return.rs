use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_body;
use ruff_python_ast::{self as ast, ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::{Edit, Fix, FixAvailability};

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
/// ## Fix safety
/// A fix is only offered for the one shape where inserting `return` cannot change anything but
/// the returned value: the method calls `super()` exactly once, and that call is its last
/// statement, so nothing can be skipped by returning there. Every other shape is reported
/// without a fix — the result stored in a variable (`res = super().write(vals)`), the call
/// made inside an `if` or a loop, more statements running after it, or a second `super()` call
/// earlier in the method — because where the `return` belongs, and what it should return, is a
/// decision only the author can make.
///
/// The fix is marked unsafe because it changes what the method returns: callers that relied on
/// the `None` of an implicit return now see the base implementation's value. That is the point
/// of the rule, but it is still a behavior change, and dropping the result is occasionally
/// deliberate.
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
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingReturn { name } = self;
        format!("Missing `return` (`super` is used) in method {name}")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Return the `super()` call".to_string())
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

/// Returns `true` if `expr` is a `super(...)` call, with or without arguments.
fn is_super_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call(ast::ExprCall { func, .. })
            if matches!(func.as_ref(), Expr::Name(name) if name.id == "super")
    )
}

/// Counts the `super(...)` calls in `body`, including the ones in nested functions and classes,
/// which is what `any_over_body` walks.
fn count_super_calls(body: &[Stmt]) -> usize {
    let mut count = 0;
    any_over_body(body, |expr| {
        if is_super_call(expr) {
            count += 1;
        }
        // Never short-circuit: every call has to be counted, not just the first one.
        false
    });
    count
}

/// Returns `true` if `expr` is a call made on `super(...)` itself, such as
/// `super().write(vals)` or `await super(MyModel, self).write(vals)`, reaching the method
/// through attribute access only. A later call in a chain (`super().create(vals).action_do()`)
/// or one that merely takes the result as an argument (`dict(super().default_get(fields))`)
/// does not qualify: what those return is no longer the base implementation's value.
fn is_super_rooted_call(expr: &Expr) -> bool {
    let expr = match expr {
        Expr::Await(ast::ExprAwait { value, .. }) => value.as_ref(),
        _ => expr,
    };
    let Expr::Call(ast::ExprCall { func, .. }) = expr else {
        return false;
    };
    let mut receiver = func.as_ref();
    loop {
        match receiver {
            Expr::Attribute(ast::ExprAttribute { value, .. }) => receiver = value.as_ref(),
            Expr::Call(ast::ExprCall { func, .. }) => {
                return matches!(func.as_ref(), Expr::Name(name) if name.id == "super");
            }
            _ => return false,
        }
    }
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

    let super_calls = count_super_calls(&function_def.body);
    if super_calls == 0 {
        return;
    }

    let is_generator = any_over_body(&function_def.body, |expr| {
        matches!(expr, Expr::Yield(_) | Expr::YieldFrom(_))
    });
    if is_generator {
        return;
    }

    if contains_return(&function_def.body) {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(
        MissingReturn {
            name: function_def.name.to_string(),
        },
        function_def.name.range(),
    );

    // Only the trailing `super()` call can be turned into a `return` by inserting the keyword:
    // anywhere else, the statements that follow it would stop running. A method calling
    // `super()` more than once is left alone too — the trailing call is then one of several
    // results, and picking the one to return is the author's call.
    if super_calls == 1
        && let Some(stmt @ Stmt::Expr(ast::StmtExpr { value, .. })) = function_def.body.last()
        && is_super_rooted_call(value)
    {
        diagnostic.set_fix(Fix::unsafe_edit(Edit::insertion(
            "return ".to_string(),
            stmt.start(),
        )));
    }
}
