use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_string_item};

/// ## What it does
/// Checks that the `version` key in an Odoo module's manifest is a five-part version whose
/// first two parts are the Odoo series, e.g. `17.0.1.0.0`.
///
/// ## Why is this bad?
/// Odoo's module updater compares manifest versions to decide whether a module has to be
/// upgraded, and the Apps store rejects modules whose version does not start with the series
/// it is published for. A version like `1.0` or `17.0.1.0` is silently treated as a different
/// (usually lower) version than intended, so upgrades stop being applied.
///
/// ## Example
/// ```python
/// {
///     "version": "1.0",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "version": "17.0.1.0.0",
/// }
/// ```
///
/// ## Options
/// - `lint.odoo.odoo-version`
///
/// When `odoo-version` is configured, the first two parts must match it exactly. Without it
/// the series is not known, so any five numeric parts are accepted.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.13")]
pub(crate) struct ManifestVersionFormat {
    version: String,
    expected: String,
}

impl Violation for ManifestVersionFormat {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestVersionFormat { version, expected } = self;
        format!(
            r#"Wrong Version Format "{version}" in manifest file. Regex to match: "{expected}""#
        )
    }
}

/// The manifest version pylint-odoo would accept, as the regex it prints in its own message,
/// so the two tools stay diff-comparable.
fn expected_format(series: Option<(u16, u16)>) -> String {
    match series {
        Some((major, minor)) => format!(r"{major}\.{minor}\.\d+\.\d+\.\d+$"),
        None => r"\d+\.\d+\.\d+\.\d+\.\d+$".to_string(),
    }
}

/// Returns `true` if `version` is five dot-separated runs of digits, the first two of which
/// match `series` when one is configured.
///
/// This is the hand-rolled equivalent of pylint-odoo's `manifest-version-format` regex.
/// Matching `\d+` means leading zeros are accepted (`17.0.01.0.0`), exactly as there.
fn version_matches(version: &str, series: Option<(u16, u16)>) -> bool {
    let mut parts = version.split('.');
    let mut next_number = || -> Option<&str> {
        parts
            .next()
            .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    };

    let (Some(major), Some(minor)) = (next_number(), next_number()) else {
        return false;
    };
    if let Some((expected_major, expected_minor)) = series
        && (major != expected_major.to_string() || minor != expected_minor.to_string())
    {
        return false;
    }
    // The three module-level components (`X.Y.Z` after the series).
    if next_number().is_none() || next_number().is_none() || next_number().is_none() {
        return false;
    }
    parts.next().is_none()
}

/// ODC8106
pub(crate) fn manifest_version_format(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    let Some((key, version)) = manifest_string_item(dict, "version") else {
        return;
    };
    // An empty version is reported by `manifest-required-key` instead.
    if version.is_empty() {
        return;
    }
    let series = checker
        .settings()
        .odoo
        .odoo_version
        .map(|odoo_version| (odoo_version.major, odoo_version.minor));
    if version_matches(version, series) {
        return;
    }
    checker.report_diagnostic(
        ManifestVersionFormat {
            version: version.to_string(),
            expected: expected_format(series),
        },
        key.range(),
    );
}
