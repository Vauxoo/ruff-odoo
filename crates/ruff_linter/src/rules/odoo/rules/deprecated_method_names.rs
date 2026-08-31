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
#[violation_metadata(preview_since = "0.16.2.2")]
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
///
/// Since Odoo 18.0 the access checks are one such case. `check_access_rights` and its
/// siblings used to be the extension points; the single hook `_check_access` replaced them,
/// so an override of the old names is dead code:
///
/// ```python
/// def check_access_rights(self, operation, raise_exception=True): ...
/// ```
///
/// Use instead:
///
/// ```python
/// def _check_access(self, operation): ...
/// ```
///
/// ## Options
/// - `lint.odoo.deprecated-odoo-model-methods`
///
/// Setting it drops the version gating the built-in list carries.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct DeprecatedOdooModelMethod {
    name: String,
    replacement: Option<String>,
}

impl Violation for DeprecatedOdooModelMethod {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DeprecatedOdooModelMethod { name, replacement } = self;
        match replacement {
            Some(replacement) => {
                format!("{name} has been deprecated by Odoo. Override `{replacement}` instead.")
            }
            None => format!("{name} has been deprecated by Odoo. Please look for alternatives."),
        }
    }
}

/// A model method Odoo no longer calls, so that overriding it is dead code.
struct DeprecatedModelMethod {
    /// The deprecated method name.
    name: &'static str,
    /// The Odoo version that deprecated it, or `None` for an entry that predates the
    /// `odoo-version` option's usefulness and is reported on every version.
    since: Option<OdooVersion>,
    /// The hook to override instead, named in the message where Odoo provides one.
    replacement: Option<&'static str>,
}

/// Model methods deprecated by Odoo.
///
/// `fields_view_get` is pylint-odoo's default set, kept ungated because it is long gone
/// (removed in 16.0). The access methods come from Odoo 18.0's access rework (commit
/// `a7450df4`, `odoo/odoo#179148`), which folded every one of them into the single
/// `_check_access` hook; they were then deleted in 20.0, so an override that survived
/// the rename is silently never called.
const DEPRECATED_MODEL_METHODS: &[DeprecatedModelMethod] = &[
    DeprecatedModelMethod {
        name: "fields_view_get",
        since: None,
        replacement: None,
    },
    DeprecatedModelMethod {
        name: "check_access_rights",
        since: Some(OdooVersion::new(18, 0)),
        replacement: Some("_check_access"),
    },
    DeprecatedModelMethod {
        name: "check_access_rule",
        since: Some(OdooVersion::new(18, 0)),
        replacement: Some("_check_access"),
    },
    DeprecatedModelMethod {
        name: "_filter_access_rules",
        since: Some(OdooVersion::new(18, 0)),
        replacement: Some("_check_access"),
    },
    DeprecatedModelMethod {
        name: "_filter_access_rules_python",
        since: Some(OdooVersion::new(18, 0)),
        replacement: Some("_check_access"),
    },
];

/// ODE8146, ODW8160
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
    {
        // The built-in entries are version-gated, so a project on 17.0 doesn't get told about
        // a method Odoo only deprecated in 18.0. A configured list replaces them wholesale and
        // is therefore reported whatever the version, as the option documents.
        let built_in: Vec<&str> = DEPRECATED_MODEL_METHODS
            .iter()
            .filter(|method| odoo_version_applies(checker, method.since, None))
            .map(|method| method.name)
            .collect();
        if checker
            .settings()
            .odoo
            .deprecated_odoo_model_methods
            .contains(function_def.name.as_str(), &built_in)
        {
            checker.report_diagnostic(
                DeprecatedOdooModelMethod {
                    name: function_def.name.to_string(),
                    replacement: DEPRECATED_MODEL_METHODS
                        .iter()
                        .find(|method| method.name == function_def.name.as_str())
                        .and_then(|method| method.replacement)
                        .map(ToString::to_string),
                },
                function_def.name.range(),
            );
        }
    }
}
