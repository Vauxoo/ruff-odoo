use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_anchor_range};

/// ## What it does
/// Checks that an Odoo module's `__manifest__.py` declares a `license` key.
///
/// ## Why is this bad?
/// Odoo uses the `license` manifest key to determine how a module may be
/// redistributed. Without it, downstream tooling (and users) can't tell
/// under what terms the module is available.
///
/// ## Example
/// ```python
/// {
///     "name": "My Module",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "name": "My Module",
///     "license": "LGPL-3",
/// }
/// ```
///
/// ## Options
/// - `lint.odoo.manifest-required-keys`
///
/// The default requires `license` alone, as pylint-odoo does.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct ManifestRequiredKey {
    key: String,
}

impl Violation for ManifestRequiredKey {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestRequiredKey { key } = self;
        format!("Missing required key \"{key}\" in manifest file")
    }
}

const REQUIRED_KEYS: &[&str] = &["license"];

/// ODC8102
pub(crate) fn manifest_required_key(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let declared: Vec<&str> = dict
        .iter_keys()
        .flatten()
        .filter_map(|dict_key| match dict_key {
            ast::Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => Some(value.to_str()),
            _ => None,
        })
        .collect();

    let required = &checker.settings().odoo.manifest_required_keys;
    for key in required.entries(REQUIRED_KEYS) {
        if !declared.contains(&key.as_ref()) {
            checker.report_diagnostic(
                ManifestRequiredKey {
                    key: key.to_string(),
                },
                manifest_anchor_range(dict),
            );
        }
    }
}
