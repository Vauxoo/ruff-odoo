use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for `from odoo.exceptions import Warning`.
///
/// ## Why is this bad?
/// `odoo.exceptions.Warning` is a deprecated alias for `odoo.exceptions.UserError`.
///
/// ## Example
/// ```python
/// from odoo.exceptions import Warning
/// ```
///
/// Use instead:
/// ```python
/// from odoo.exceptions import UserError
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct OdooExceptionWarning;

impl Violation for OdooExceptionWarning {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`odoo.exceptions.Warning` is a deprecated alias to `odoo.exceptions.UserError`, use \
         `from odoo.exceptions import UserError`"
            .to_string()
    }
}

/// ODOO020
pub(crate) fn odoo_exception_warning(checker: &Checker, import_from: &ast::StmtImportFrom) {
    if import_from.module.as_deref() != Some("odoo.exceptions") {
        return;
    }
    if import_from
        .names
        .iter()
        .any(|alias| alias.name.as_str() == "Warning")
    {
        checker.report_diagnostic(OdooExceptionWarning, import_from.range());
    }
}
