use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_anchor_range, manifest_item};

/// ## What it does
/// Checks that an Odoo module's `__manifest__.py` declares one of the required authors.
///
/// ## Why is this bad?
/// Requiring a known author (or organization) in the manifest helps track module provenance.
///
/// ## Example
/// ```python
/// {
///     "author": "Someone Else",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct ManifestRequiredAuthor {
    required: &'static str,
}

impl Violation for ManifestRequiredAuthor {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestRequiredAuthor { required } = self;
        format!("One of the following authors must be present in manifest: '{required}'")
    }
}

const REQUIRED_AUTHORS: &[&str] = &["Odoo Community Association (OCA)"];

/// ODC8101
pub(crate) fn manifest_required_author(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let (report_range, author) = match manifest_item(dict, "author") {
        Some((key, Expr::StringLiteral(ast::ExprStringLiteral { value, .. }))) => {
            (key.range(), value.to_str())
        }
        Some(_) => return, // Non-string author; `manifest-author-string` handles that case.
        None => (manifest_anchor_range(dict), ""),
    };

    let authors: std::collections::HashSet<&str> = author.split(',').map(str::trim).collect();
    if !REQUIRED_AUTHORS
        .iter()
        .any(|required| authors.contains(required))
    {
        checker.report_diagnostic(
            ManifestRequiredAuthor {
                required: REQUIRED_AUTHORS[0],
            },
            report_range,
        );
    }
}
