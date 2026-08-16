use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for `TransientModel` (wizard) classes defined inside the `models/` directory of
/// an Odoo module.
///
/// ## Why is this bad?
/// The OCA module structure keeps wizards in the `wizards/` directory; mixing them into
/// `models/` makes the module layout harder to navigate.
///
/// Classes extending the settings screens (`_inherit` starting with `res.config`) are
/// exempt: those conventionally live with the models.
///
/// ## Example
/// A `models/sale_import.py` file containing:
/// ```python
/// class SaleImport(models.TransientModel):
///     _name = "sale.import"
/// ```
///
/// Use instead a `wizards/sale_import.py` file containing:
/// ```python
/// class SaleImport(models.TransientModel):
///     _name = "sale.import"
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.5")]
pub(crate) struct NoWizardInModels;

impl Violation for NoWizardInModels {
    #[derive_message_formats]
    fn message(&self) -> String {
        "No wizard class for model directory. See the complete structure \
         https://github.com/OCA/odoo-community.org/blob/master/website/Contribution/CONTRIBUTING.rst#complete-structure"
            .to_string()
    }
}

/// Returns `true` if the class body assigns `_inherit` a string starting with `prefix`.
fn inherits_with_prefix(class_def: &ast::StmtClassDef, prefix: &str) -> bool {
    class_def.body.iter().any(|stmt| {
        let Stmt::Assign(ast::StmtAssign { targets, value, .. }) = stmt else {
            return false;
        };
        if !targets
            .iter()
            .any(|target| matches!(target, Expr::Name(name) if name.id == "_inherit"))
        {
            return false;
        }
        matches!(
            value.as_ref(),
            Expr::StringLiteral(ast::ExprStringLiteral { value, .. })
                if value.to_str().starts_with(prefix)
        )
    })
}

/// ODC8113
pub(crate) fn no_wizard_in_models(checker: &Checker, class_def: &ast::StmtClassDef, path: &Path) {
    // Matching pylint-odoo: any directory name starting with "model" (models, model, ...).
    if !path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("model"))
    {
        return;
    }
    let Some(arguments) = class_def.arguments.as_deref() else {
        return;
    };
    let Some(base) = arguments.args.iter().find(|base| {
        let name = match base {
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str(),
            Expr::Name(ast::ExprName { id, .. }) => id.as_str(),
            _ => return false,
        };
        name == "TransientModel"
    }) else {
        return;
    };
    // Settings wizards (res.config.settings and friends) conventionally live in models/.
    if inherits_with_prefix(class_def, "res.config") {
        return;
    }
    checker.report_diagnostic(NoWizardInModels, base.range());
}
