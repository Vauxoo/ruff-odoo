use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item};

/// ## What it does
/// Checks that the `maintainers` key in an Odoo module's `__manifest__.py`, if present, is a
/// list of strings.
///
/// ## Why is this bad?
/// Odoo expects `maintainers` to be a list of GitHub usernames (strings).
///
/// ## Example
/// ```python
/// {
///     "maintainers": "someone",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "maintainers": ["someone"],
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct ManifestMaintainersList;

impl Violation for ManifestMaintainersList {
    #[derive_message_formats]
    fn message(&self) -> String {
        "The maintainers key in the manifest file must be a list of strings".to_string()
    }
}

/// ODOO011
pub(crate) fn manifest_maintainers_list(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let Some((key, value)) = manifest_item(dict, "maintainers") else {
        return;
    };
    // Mirrors pylint-odoo's `if maintainers and (...)`: a falsy value (`None`, `""`) is exempt.
    if matches!(value, Expr::NoneLiteral(_))
        || matches!(value, Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) if value.to_str().is_empty())
    {
        return;
    }
    let Expr::List(ast::ExprList { elts, .. }) = value else {
        checker.report_diagnostic(ManifestMaintainersList, key.range());
        return;
    };
    if elts
        .iter()
        .any(|elt| !matches!(elt, Expr::StringLiteral(_)))
    {
        checker.report_diagnostic(ManifestMaintainersList, key.range());
    }
}
