use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_anchor_range, manifest_item};

/// ## What it does
/// Checks that the `__manifest__.py` of an Odoo module with a `price` key (a paid app)
/// declares the extra keys the Odoo Apps store requires: `currency`, `images`, and
/// `support`.
///
/// ## Why is this bad?
/// Paid apps are published on [apps.odoo.com](https://apps.odoo.com/apps), which uses
/// `currency` to price the app, `images` for the store listing, and `support` as the
/// contact for buyers. Without them the store listing is incomplete.
///
/// ## Example
/// ```python
/// {
///     "name": "My App",
///     "price": 100,
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "name": "My App",
///     "price": 100,
///     "currency": "EUR",
///     "images": ["static/description/banner.png"],
///     "support": "support@example.com",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.10")]
pub(crate) struct ManifestRequiredKeyApp {
    key: &'static str,
}

impl Violation for ManifestRequiredKeyApp {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestRequiredKeyApp { key } = self;
        format!("Missing required key \"{key}\" in manifest file for modules with price")
    }
}

// `license` is also required for apps, but ODOO001 already requires it for every module.
const REQUIRED_KEYS_APP: &[&str] = &["currency", "images", "support"];

/// OAPP003
pub(crate) fn manifest_required_key_app(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    if manifest_item(dict, "price").is_none() {
        return;
    }
    for key in REQUIRED_KEYS_APP {
        if manifest_item(dict, key).is_none() {
            checker.report_diagnostic(ManifestRequiredKeyApp { key }, manifest_anchor_range(dict));
        }
    }
}
