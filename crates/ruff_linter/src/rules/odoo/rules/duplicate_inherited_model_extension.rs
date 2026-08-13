use ruff_macros::{ViolationMetadata, derive_message_formats};

use crate::Violation;

/// ## What it does
/// Checks for multiple Odoo model extension classes in the same module that use
/// the same `_inherit` model name.
///
/// ## Why is this bad?
/// Splitting the extension of a single Odoo model across several classes in the
/// same module makes the module harder to read and maintain. In most cases, the
/// classes can be merged into one extension class for that inherited model.
///
/// ## Example
/// ```python
/// class ResPartnerA(models.Model):
///     _inherit = "res.partner"
///
///
/// class ResPartnerB(models.Model):
///     _inherit = "res.partner"
/// ```
///
/// Use instead:
/// ```python
/// class ResPartner(models.Model):
///     _inherit = "res.partner"
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DuplicateInheritedModelExtension {
    pub(crate) model: String,
    pub(crate) locations: String,
}

impl Violation for DuplicateInheritedModelExtension {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DuplicateInheritedModelExtension { model, locations } = self;
        format!("Consider merging classes inherited to \"{model}\" from {locations}.")
    }
}
