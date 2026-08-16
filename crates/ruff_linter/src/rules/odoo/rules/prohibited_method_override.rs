use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_body;
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for overrides of methods listed in the `lint.odoo.prohibited-override-methods`
/// setting: methods that delegate to `super().<method>(...)` while their name is on the
/// prohibited list.
///
/// ## Why is this bad?
/// Some projects mark specific ORM or business methods as sealed: overriding them has
/// caused regressions before, or the project wants all changes to go through other
/// extension points (e.g. dedicated hooks) instead.
///
/// ## Example
/// Given `prohibited-override-methods = ["action_post"]`:
///
/// ```python
/// class AccountMove(models.Model):
///     _inherit = "account.move"
///
///     def action_post(self):
///         return super().action_post()
/// ```
///
/// ## Options
/// - `lint.odoo.prohibited-override-methods`
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.8")]
pub(crate) struct ProhibitedMethodOverride {
    name: String,
}

impl Violation for ProhibitedMethodOverride {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ProhibitedMethodOverride { name } = self;
        format!("Prohibited override of \"{name}\" method")
    }
}

/// ODW8107
pub(crate) fn prohibited_method_override(checker: &Checker, function_def: &ast::StmtFunctionDef) {
    if !checker.semantic().current_scope().kind.is_class() {
        return;
    }
    let name = function_def.name.as_str();
    if !checker
        .settings()
        .odoo
        .prohibited_override_methods
        .iter()
        .any(|method| method == name)
    {
        return;
    }

    let overrides_super = any_over_body(&function_def.body, |expr| {
        matches!(
            expr,
            Expr::Attribute(ast::ExprAttribute { value, attr, .. })
                if attr.as_str() == name
                    && matches!(
                        value.as_ref(),
                        Expr::Call(ast::ExprCall { func, .. })
                            if matches!(func.as_ref(), Expr::Name(func_name) if func_name.id == "super")
                    )
        )
    });
    if overrides_super {
        checker.report_diagnostic(
            ProhibitedMethodOverride {
                name: name.to_string(),
            },
            function_def.name.range(),
        );
    }
}
