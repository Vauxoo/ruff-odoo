use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item};
use crate::{Edit, Fix, FixAvailability, Violation};

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
///
/// ## Fix safety
/// The fix wraps a single string in a list, and rewrites a tuple of strings as a list. It is
/// marked as unsafe because other tooling reading the manifest may expect the original
/// value. No fix is offered for other values, or for a string containing a comma — that
/// probably names several maintainers, and how to split them is a judgment call.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct ManifestMaintainersList;

impl Violation for ManifestMaintainersList {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "The maintainers key in the manifest file must be a list of strings".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Turn the value into a list of strings".to_string())
    }
}

/// ODE8104
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
        let mut diagnostic = checker.report_diagnostic(ManifestMaintainersList, key.range());
        if let Some(replacement) = listified(checker, value) {
            diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                replacement,
                value.range(),
            )));
        }
        return;
    };
    if elts
        .iter()
        .any(|elt| !matches!(elt, Expr::StringLiteral(_)))
    {
        checker.report_diagnostic(ManifestMaintainersList, key.range());
    }
}

/// The value rewritten as a list literal, when that rewrite is obviously right: a single
/// comma-free string becomes a one-element list, and a tuple of strings swaps its
/// parentheses for brackets.
fn listified(checker: &Checker, value: &Expr) -> Option<String> {
    match value {
        Expr::StringLiteral(ast::ExprStringLiteral { value: literal, .. }) => {
            // A comma suggests several maintainers crammed into one string; splitting them
            // is a judgment call the fix stays out of.
            if literal.to_str().contains(',') {
                return None;
            }
            Some(format!("[{}]", checker.locator().slice(value.range())))
        }
        Expr::Tuple(ast::ExprTuple { elts, .. }) => {
            if !elts.iter().all(|elt| matches!(elt, Expr::StringLiteral(_))) {
                return None;
            }
            let (Some(first), Some(last)) = (elts.first(), elts.last()) else {
                return Some("[]".to_string());
            };
            let inner = checker
                .locator()
                .slice(TextRange::new(first.start(), last.end()));
            Some(format!("[{inner}]"))
        }
        _ => None,
    }
}
