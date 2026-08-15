use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for `super().other_method()` calls where the method called on `super()` differs
/// from the enclosing method's own name.
///
/// ## Why is this bad?
/// Calling a *different* method on `super()` is usually a copy-paste mistake; when it's
/// intentional, it tends to surprise readers and break cooperative multiple inheritance.
///
/// ## Example
/// ```python
/// def write(self, vals):
///     return super().create(vals)
/// ```
///
/// Use instead:
/// ```python
/// def write(self, vals):
///     return super().write(vals)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct SuperMethodMismatch {
    called: String,
    defined: String,
}

impl Violation for SuperMethodMismatch {
    #[derive_message_formats]
    fn message(&self) -> String {
        let SuperMethodMismatch { called, defined } = self;
        format!("`super().{called}` mismatch but defined method is `{defined}`")
    }
}

/// ODOO036
pub(crate) fn super_method_mismatch(checker: &Checker, call: &ast::ExprCall) {
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
        return;
    };
    let Expr::Call(inner) = value.as_ref() else {
        return;
    };
    if !matches!(inner.func.as_ref(), Expr::Name(name) if name.id == "super") {
        return;
    }

    let ScopeKind::Function(function_def) = checker.semantic().current_scope().kind else {
        return;
    };
    // Only inside methods: some ancestor scope must be a class.
    if !checker
        .semantic()
        .current_scopes()
        .any(|scope| scope.kind.is_class())
    {
        return;
    }

    let defined = function_def.name.as_str();
    // Job-queue and cache wrappers intentionally delegate to a differently-named method.
    if attr == defined || defined.contains("queue") || defined.contains("cache") {
        return;
    }
    checker.report_diagnostic(
        SuperMethodMismatch {
            called: attr.to_string(),
            defined: defined.to_string(),
        },
        call.range(),
    );
}
