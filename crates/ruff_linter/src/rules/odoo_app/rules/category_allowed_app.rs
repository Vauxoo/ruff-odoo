use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item, manifest_string_item};

/// ## What it does
/// Checks that the `category` in the `__manifest__.py` of an Odoo module with a `price`
/// key (a paid app) is one of the categories listed on the Odoo Apps store.
///
/// ## Why is this bad?
/// Paid apps are published on [apps.odoo.com](https://apps.odoo.com/apps), which organizes
/// modules by a fixed set of categories. A category outside that set won't match any store
/// section, making the app harder to find.
///
/// ## Example
/// ```python
/// {
///     "name": "My App",
///     "price": 100,
///     "category": "My Custom Category",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "name": "My App",
///     "price": 100,
///     "category": "Sales",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct CategoryAllowedApp {
    category: String,
}

impl Violation for CategoryAllowedApp {
    #[derive_message_formats]
    fn message(&self) -> String {
        let CategoryAllowedApp { category } = self;
        format!("Category \"{category}\" not allowed in manifest file for modules with price")
    }
}

// Based on https://apps.odoo.com/apps
const CATEGORY_ALLOWED_APP: &[&str] = &[
    "Accounting",
    "Discuss",
    "Document Management",
    "eCommerce",
    "Extra Tools",
    "Human Resources",
    "Industries",
    "Localization",
    "Manufacturing",
    "Marketing",
    "Point of Sale",
    "Productivity",
    "Project",
    "Purchases",
    "Sales",
    "Tutorial",
    "Warehouse",
    "Website",
];

/// OAPP001
pub(crate) fn category_allowed_app(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    if manifest_item(dict, "price").is_none() {
        return;
    }
    let Some((key, category)) = manifest_string_item(dict, "category") else {
        return;
    };
    if category.is_empty() || CATEGORY_ALLOWED_APP.contains(&category) {
        return;
    }
    checker.report_diagnostic(
        CategoryAllowedApp {
            category: category.to_string(),
        },
        key.range(),
    );
}
