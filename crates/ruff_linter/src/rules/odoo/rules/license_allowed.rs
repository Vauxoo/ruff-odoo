use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_file, manifest_string_item};

/// ## What it does
/// Checks that the `license` key in an Odoo module's `__manifest__.py` is one of the
/// commonly-accepted OSI/OCA license identifiers.
///
/// ## Why is this bad?
/// An unrecognized license string usually indicates a typo, and tooling that reads the
/// manifest (including Odoo itself) won't recognize it.
///
/// ## Example
/// ```python
/// {
///     "license": "GPL",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct LicenseAllowed {
    license: String,
}

impl Violation for LicenseAllowed {
    #[derive_message_formats]
    fn message(&self) -> String {
        let LicenseAllowed { license } = self;
        format!("License \"{license}\" not allowed in manifest file")
    }
}

const LICENSE_ALLOWED: &[&str] = &[
    "AGPL-3",
    "GPL-2 or any later version",
    "GPL-2",
    "GPL-3 or any later version",
    "GPL-3",
    "LGPL-3",
    "OEEL-1",
    "Other OSI approved licence",
    "Other proprietary",
];

/// ODOO010
pub(crate) fn license_allowed(checker: &Checker, dict: &ast::ExprDict, path: &std::path::Path) {
    if !is_manifest_file(path) {
        return;
    }
    if !checker.semantic().current_scope().kind.is_module() {
        return;
    }

    let Some((key, license)) = manifest_string_item(dict, "license") else {
        return;
    };
    if license.is_empty() || LICENSE_ALLOWED.contains(&license) {
        return;
    }
    checker.report_diagnostic(
        LicenseAllowed {
            license: license.to_string(),
        },
        key.range(),
    );
}
