use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for `name_get` method definitions.
///
/// ## Why is this bad?
/// `name_get` is deprecated since Odoo 17.0; the display name is computed by
/// `_compute_display_name` instead.
///
/// ## Example
///
/// ```python
/// def name_get(self): ...
/// ```
///
/// Use instead:
///
/// ```python
/// def _compute_display_name(self): ...
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DeprecatedNameGet;

impl Violation for DeprecatedNameGet {
    #[derive_message_formats]
    fn message(&self) -> String {
        "'name_get' is deprecated. Use '_compute_display_name' instead".to_string()
    }
}

/// ## What it does
/// Checks for overrides of model methods that Odoo has deprecated (e.g. `fields_view_get`,
/// removed in 16.0).
///
/// ## Why is this bad?
/// Overriding a method the ORM no longer calls means the override silently never runs.
///
/// ## Example
///
/// ```python
/// def fields_view_get(self, view_id=None, view_type="form", **kwargs): ...
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DeprecatedOdooModelMethod {
    name: String,
}

impl Violation for DeprecatedOdooModelMethod {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DeprecatedOdooModelMethod { name } = self;
        format!("{name} has been deprecated by Odoo. Please look for alternatives.")
    }
}

/// Model methods deprecated by Odoo (as of 16.0+, matching pylint-odoo's default set).
const DEPRECATED_MODEL_METHODS: &[&str] = &["fields_view_get"];

/// ODOO034, ODOO037
pub(crate) fn deprecated_method_names(checker: &Checker, function_def: &ast::StmtFunctionDef) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };

    if checker.is_rule_enabled(Rule::DeprecatedNameGet)
        && function_def.name.as_str() == "name_get"
        // `name_get` was only deprecated in Odoo 17.0.
        && odoo_version_applies(checker, Some(OdooVersion::new(17, 0)), None)
    {
        checker.report_diagnostic(DeprecatedNameGet, function_def.name.range());
    }

    if checker.is_rule_enabled(Rule::DeprecatedOdooModelMethod)
        && is_odoo_model_class(checker.semantic(), class_def)
        && DEPRECATED_MODEL_METHODS.contains(&function_def.name.as_str())
    {
        checker.report_diagnostic(
            DeprecatedOdooModelMethod {
                name: function_def.name.to_string(),
            },
            function_def.name.range(),
        );
    }
}
