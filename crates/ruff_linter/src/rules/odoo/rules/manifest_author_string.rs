use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item};

/// ## What it does
/// Checks that the `author` key in an Odoo module's `__manifest__.py` is a string.
///
/// ## Why is this bad?
/// Odoo expects `author` to be a single string of comma-separated author names, not a list.
///
/// ## Example
/// ```python
/// {
///     "author": ["My Company"],
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "author": "My Company",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ManifestAuthorString;

impl Violation for ManifestAuthorString {
    #[derive_message_formats]
    fn message(&self) -> String {
        "The author key in the manifest file must be a string (with comma separated values)"
            .to_string()
    }
}

/// ODOO009
pub(crate) fn manifest_author_string(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let Some((key, value)) = manifest_item(dict, "author") else {
        return;
    };
    if matches!(value, Expr::StringLiteral(_)) {
        return;
    }
    checker.report_diagnostic(ManifestAuthorString, key.range());
}
