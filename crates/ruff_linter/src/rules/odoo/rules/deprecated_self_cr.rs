use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability};

/// ## What it does
/// Checks for `self._cr` usage in Odoo model classes.
///
/// ## Why is this bad?
/// `self._cr` is deprecated since Odoo 19.0 in favor of `self.env.cr`.
///
/// ## Example
/// ```python
/// self._cr.execute("SELECT 1")
/// ```
///
/// Use instead:
/// ```python
/// self.env.cr.execute("SELECT 1")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DeprecatedSelfCr;

impl Violation for DeprecatedSelfCr {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Use \"self.env.cr\" instead of \"self._cr\" (deprecated since 19.0)".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Replace with `self.env.cr`".to_string())
    }
}

/// ODOO035
pub(crate) fn deprecated_self_cr(checker: &Checker, attribute: &ast::ExprAttribute) {
    // `self._cr` was only deprecated in Odoo 19.0.
    if !odoo_version_applies(checker, Some(OdooVersion::new(19, 0)), None) {
        return;
    }
    if attribute.attr.as_str() != "_cr" {
        return;
    }
    if !matches!(attribute.value.as_ref(), Expr::Name(name) if name.id == "self") {
        return;
    }
    let semantic = checker.semantic();
    let in_odoo_model = semantic.current_scopes().any(
        |scope| matches!(scope.kind, ScopeKind::Class(class_def) if is_odoo_model_class(semantic, class_def)),
    );
    if !in_odoo_model {
        return;
    }
    let mut diagnostic = checker.report_diagnostic(DeprecatedSelfCr, attribute.range());
    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        "self.env.cr".to_string(),
        attribute.range(),
    )));
}
