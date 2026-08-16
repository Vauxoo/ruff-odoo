use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_body;
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks that common Odoo ORM methods (`create`, `write`, `unlink`, ...) call `super()`
/// somewhere in their body.
///
/// ## Why is this bad?
/// Overriding one of these methods without calling `super()` usually means the base
/// implementation (and any other module's override in the resolution order) never
/// runs, silently breaking the inheritance chain.
///
/// ## Example
/// ```python
/// class MyModel(models.Model):
///     _inherit = "my.model"
///
///     def write(self, vals):
///         return True
/// ```
///
/// Use instead:
/// ```python
/// class MyModel(models.Model):
///     _inherit = "my.model"
///
///     def write(self, vals):
///         return super().write(vals)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct MethodRequiredSuper {
    name: String,
}

impl Violation for MethodRequiredSuper {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MethodRequiredSuper { name } = self;
        format!("Missing `super` call in \"{name}\" method")
    }
}

const METHODS_REQUIRING_SUPER: &[&str] = &[
    "copy",
    "create",
    "default_get",
    "read",
    "setUp",
    "setUpClass",
    "tearDown",
    "tearDownClass",
    "unlink",
    "write",
];

/// ODW8106
pub(crate) fn method_required_super(checker: &Checker, function_def: &ast::StmtFunctionDef) {
    if !checker.semantic().current_scope().kind.is_class() {
        return;
    }
    if !METHODS_REQUIRING_SUPER.contains(&function_def.name.as_str()) {
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
        checker.report_diagnostic(
            MethodRequiredSuper {
                name: function_def.name.to_string(),
            },
            function_def.name.range(),
        );
    }
}
