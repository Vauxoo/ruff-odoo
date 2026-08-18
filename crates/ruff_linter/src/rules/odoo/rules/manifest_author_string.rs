use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item};
use crate::{Edit, Fix, FixAvailability, Violation};

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
///
/// ## Fix safety
/// When the value is a list or tuple of plain string literals, the fix joins them into one
/// comma-separated string, which is how Odoo reads the key. It is marked as unsafe because
/// other tooling reading the manifest may expect the sequence. No fix is offered for any
/// other value, or when an author name itself contains a comma, quote, or backslash.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct ManifestAuthorString;

impl Violation for ManifestAuthorString {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "The author key in the manifest file must be a string (with comma separated values)"
            .to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Join the authors into one string".to_string())
    }
}

/// ODE8101
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
    let mut diagnostic = checker.report_diagnostic(ManifestAuthorString, key.range());

    let (Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. })) = value
    else {
        return;
    };
    let Some(authors) = elts
        .iter()
        .map(|elt| match elt {
            Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => Some(value.to_str()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    // A comma inside a name would blur into the separators; a quote or backslash would need
    // escaping the joined literal can't be trusted to preserve.
    if authors
        .iter()
        .any(|author| author.contains([',', '"', '\'', '\\']))
    {
        return;
    }
    let quote = checker.stylist().quote();
    diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
        format!("{quote}{}{quote}", authors.join(", ")),
        value.range(),
    )));
}
