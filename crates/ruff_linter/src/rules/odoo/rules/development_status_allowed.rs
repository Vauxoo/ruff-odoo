use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_string_item};

/// ## What it does
/// Checks that the `development_status` key in an Odoo module's `__manifest__.py`, if
/// present, is one of the recognized values.
///
/// ## Why is this bad?
/// An unrecognized development status is usually a typo and won't be understood by tooling
/// that reads the manifest.
///
/// ## Example
/// ```python
/// {
///     "development_status": "Stable",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "development_status": "Production/Stable",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DevelopmentStatusAllowed {
    status: String,
}

impl Violation for DevelopmentStatusAllowed {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DevelopmentStatusAllowed { status } = self;
        format!(
            "Manifest key development_status \"{status}\" not allowed. Use one of: Alpha, Beta, Mature, Production/Stable."
        )
    }
}

const DEVELOPMENT_STATUS_ALLOWED: &[&str] = &["Alpha", "Beta", "Mature", "Production/Stable"];

/// ODOO013
pub(crate) fn development_status_allowed(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let Some((key, status)) = manifest_string_item(dict, "development_status") else {
        return;
    };
    if status.is_empty() || DEVELOPMENT_STATUS_ALLOWED.contains(&status) {
        return;
    }
    checker.report_diagnostic(
        DevelopmentStatusAllowed {
            status: status.to_string(),
        },
        key.range(),
    );
}
