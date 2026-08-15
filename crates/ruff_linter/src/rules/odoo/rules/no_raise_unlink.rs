use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for `raise` statements inside `unlink()` methods of Odoo models.
///
/// ## Why is this bad?
/// Since Odoo 15.0, deletion constraints belong in an `@api.ondelete`-decorated method;
/// raising inside `unlink` itself breaks batch deletions from other modules that expect
/// `unlink` to succeed after their own `@api.ondelete` checks passed.
///
/// ## Example
/// ```python
/// def unlink(self):
///     if self.state == "done":
///         raise UserError("Cannot delete a done record")
///     return super().unlink()
/// ```
///
/// Use instead:
/// ```python
/// @api.ondelete(at_uninstall=False)
/// def _unlink_except_done(self):
///     if self.state == "done":
///         raise UserError("Cannot delete a done record")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct NoRaiseUnlink;

impl Violation for NoRaiseUnlink {
    #[derive_message_formats]
    fn message(&self) -> String {
        "No exceptions should be raised inside unlink() functions".to_string()
    }
}

/// Returns `true` if the class body assigns `_name` or `_inherit` (the marker of a real
/// Odoo model definition, matching pylint-odoo's `_is_unlink`).
fn class_has_model_attributes(class_def: &ast::StmtClassDef) -> bool {
    class_def.body.iter().any(|stmt| {
        let Stmt::Assign(assign) = stmt else {
            return false;
        };
        assign.targets.iter().any(|target| {
            matches!(target, Expr::Name(name) if name.id == "_name" || name.id == "_inherit")
        })
    })
}

/// ODOO039
pub(crate) fn no_raise_unlink(checker: &Checker, raise: &ast::StmtRaise) {
    // Deletion constraints only moved out of `unlink()` in Odoo 15.0.
    if !odoo_version_applies(checker, Some(OdooVersion::new(15, 0)), None) {
        return;
    }
    let mut scopes = checker.semantic().current_scopes();
    let in_unlink_method = scopes.any(|scope| {
        matches!(scope.kind, ScopeKind::Function(function_def) if function_def.name.as_str() == "unlink")
    });
    if !in_unlink_method {
        return;
    }
    // After `any()` consumed up to the function scope, the remaining scopes are its
    // ancestors — the class (if any) is among them.
    let in_model_class = scopes.any(|scope| {
        matches!(scope.kind, ScopeKind::Class(class_def) if class_has_model_attributes(class_def))
    });
    if !in_model_class {
        return;
    }
    checker.report_diagnostic(NoRaiseUnlink, raise.range());
}
