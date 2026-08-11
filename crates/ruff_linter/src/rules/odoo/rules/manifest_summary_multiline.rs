use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_file, manifest_string_item};

/// ## What it does
/// Checks that the `summary` key in an Odoo module's `__manifest__.py` is a single line.
///
/// ## Why is this bad?
/// Odoo displays `summary` as a one-line short description in the Apps list; a newline
/// breaks that layout.
///
/// ## Example
/// ```python
/// {
///     "summary": "Does\nthings.",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ManifestSummaryMultiline;

impl Violation for ManifestSummaryMultiline {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Summary in manifest file should be a one-line short description, found newline character"
            .to_string()
    }
}

/// ODOO012
pub(crate) fn manifest_summary_multiline(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_file(path) {
        return;
    }
    if !checker.semantic().current_scope().kind.is_module() {
        return;
    }

    let Some((key, summary)) = manifest_string_item(dict, "summary") else {
        return;
    };
    if summary.contains('\n') {
        checker.report_diagnostic(ManifestSummaryMultiline, key.range());
    }
}
