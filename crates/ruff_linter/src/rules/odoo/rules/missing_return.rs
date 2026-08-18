use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::{any_over_body, any_over_expr};
use ruff_python_ast::{self as ast, ExceptHandler, Expr, Stmt};
use ruff_python_trivia::indentation_at_offset;
use ruff_source_file::LineRanges;
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
/// Every fix requires the method to call `super()` exactly once: with two calls, the value to
/// hand back is a choice between them, and choosing is the author's job. Given that single
/// call, there are two shapes where what to return is not a guess:
///
/// - the call is the method's last statement, and the `return` goes in front of it. Nothing
///   can be skipped by returning there, because nothing runs after it.
/// - the call's result is assigned to a plain variable (`res = super().default_get(fields)`) at
///   the top level of the method, and `return res` is appended at the end. Whatever the
///   method does to `res` in between, `res` is the value it was building.
///
/// Anything else keeps the bare report: the call made inside an `if` or a loop, or its result
/// assigned there, where the name may never be bound; the assignment target a tuple, an
/// attribute or a subscript rather than a plain name; a method ending in `raise`, where the
/// `return` would be dead code; and a call further down a chain, whose value is no longer the
/// base implementation's.
///
/// The fix is marked unsafe because it changes what the method returns: callers that relied on
/// the `None` of an implicit return now see the value the method was building. That is the
/// point of the rule, but it is still a behavior change, and dropping the result is
/// occasionally deliberate.
///
/// ## Options
/// - `lint.odoo.no-missing-return`
///
/// Names the methods exempt from returning, not the ones checked.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct MissingReturn {
    name: String,
    /// The variable the fix returns, for the shape that assigns the `super()` result to one.
    /// `None` when the fix returns the call itself, and when there is no fix to offer.
    returned: Option<String>,
}

impl Violation for MissingReturn {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingReturn { name, .. } = self;
        format!("Missing `return` (`super` is used) in method {name}")
    }

    fn fix_title(&self) -> Option<String> {
        Some(match &self.returned {
            Some(returned) => format!("Return `{returned}`"),
            None => "Return the `super()` call".to_string(),
        })
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

    // A method calling `super()` more than once has no single result to hand back, so the
    // `return` it is missing is one only its author can write.
    let fix = (super_calls == 1)
        .then(|| missing_return_fix(checker, function_def))
        .flatten();

    let mut diagnostic = checker.report_diagnostic(
        MissingReturn {
            name: function_def.name.to_string(),
            returned: fix
                .as_ref()
                .and_then(|(_, returned)| returned.as_ref().map(ToString::to_string)),
        },
        function_def.name.range(),
    );
    if let Some((fix, _)) = fix {
        diagnostic.set_fix(fix);
    }
}

/// Builds the `return` for the two shapes where the value to hand back is not a guess, together
/// with the name it returns, if it returns one rather than the `super()` call itself. Only
/// called for a method with exactly one `super()` call.
fn missing_return_fix<'a>(
    checker: &Checker,
    function_def: &'a ast::StmtFunctionDef,
) -> Option<(Fix, Option<&'a str>)> {
    let last = function_def.body.last()?;

    // `super().write(vals)` as the last statement: the keyword goes in front of the call.
    if let Stmt::Expr(ast::StmtExpr { value, .. }) = last
        && is_super_rooted_call(value)
    {
        return Some((
            Fix::unsafe_edit(Edit::insertion("return ".to_string(), last.start())),
            None,
        ));
    }

    // `res = super().default_get(fields)`: `return res` goes at the end of the method, however
    // many statements the method spends building `res` after the call.
    let name = assigned_super_result(&function_def.body)?;

    // A method ending in `raise` never returns normally, so the `return` would be dead code.
    if last.is_raise_stmt() {
        return None;
    }

    let locator = checker.locator();
    // The last statement's own indentation is the body's, unless it shares its line with
    // another one (`res["name"] = "x"; do()`), where there is none to copy.
    let indentation = indentation_at_offset(last.start(), locator.contents())?;
    let line_ending = checker.stylist().line_ending().as_str();
    let insertion = locator.full_line_end(last.end());
    let mut content = format!("{indentation}return {name}{line_ending}");
    // A method ending the file on a line without a terminator needs one of its own first.
    if !locator.up_to(insertion).ends_with(['\n', '\r']) {
        content.insert_str(0, line_ending);
    }
    Some((
        Fix::unsafe_edit(Edit::insertion(content, insertion)),
        Some(name),
    ))
}

/// Returns the name a top-level statement of `body` assigns the `super()` result to, as in
/// `res = super().default_get(fields)`.
///
/// Only a plain single-name target counts — a tuple, an attribute or a subscript is not a value
/// the method can hand back by name — and only at the top level of the method: a name bound
/// inside an `if` or a loop may not be bound at all by the time the method ends.
fn assigned_super_result(body: &[Stmt]) -> Option<&str> {
    body.iter().find_map(|stmt| {
        let ast::StmtAssign { targets, value, .. } = stmt.as_assign_stmt()?;
        let [Expr::Name(target)] = targets.as_slice() else {
            return None;
        };
        any_over_expr(value, &is_super_call).then(|| target.id.as_str())
    })
}
