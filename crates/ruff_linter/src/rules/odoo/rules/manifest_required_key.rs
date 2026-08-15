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
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct ManifestRequiredKey {
    key: &'static str,
}

impl Violation for ManifestRequiredKey {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestRequiredKey { key } = self;
        format!("Missing required key \"{key}\" in manifest file")
    }
}

/// ODOO001
pub(crate) fn manifest_required_key(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let key = "license";
    let has_key = dict.iter_keys().flatten().any(|dict_key| {
        matches!(
            dict_key,
            ast::Expr::StringLiteral(ast::ExprStringLiteral { value, .. })
                if value.to_str() == key
        )
    });
    if !has_key {
        checker.report_diagnostic(ManifestRequiredKey { key }, manifest_anchor_range(dict));
    }
}
