use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_string_item};

/// ## What it does
/// Checks that the `category` in an Odoo module's manifest is one of the categories the
/// project allows.
///
/// ## Why is this bad?
/// Categories group modules in the Apps list. Teams that standardize on a fixed set of
/// categories end up with modules scattered across near-duplicate categories ("Accounting"
/// vs "Account" vs "Finance") when typos and one-off names go unnoticed.
///
/// ## Example
/// ```python
/// {
///     "name": "My Module",
///     "category": "Acounting",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "name": "My Module",
///     "category": "Accounting",
/// }
/// ```
///
/// ## Options
/// - `lint.odoo.category-allowed`
///
/// The rule is inert until `category-allowed` lists at least one category, mirroring
/// pylint-odoo, whose `--category-allowed` also defaults to an empty list. Paid apps have a
/// separate rule with the store's own fixed list, `category-allowed-app` (`OAPP001`).
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct CategoryAllowed {
    category: String,
}

impl Violation for CategoryAllowed {
    #[derive_message_formats]
    fn message(&self) -> String {
        let CategoryAllowed { category } = self;
        format!("Category \"{category}\" not allowed in manifest file")
    }
}

/// ODOO065
pub(crate) fn category_allowed(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    let allowed = &checker.settings().odoo.category_allowed;
    if allowed.is_empty() {
        return;
    }
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    let Some((key, category)) = manifest_string_item(dict, "category") else {
        return;
    };
    if category.is_empty() || allowed.iter().any(|entry| entry == category) {
        return;
    }
    checker.report_diagnostic(
        CategoryAllowed {
            category: category.to_string(),
        },
        key.range(),
    );
}
