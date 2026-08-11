use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, str_prefix::StringLiteralPrefix};
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_file, manifest_item};
use crate::{AlwaysFixableViolation, Edit, Fix};

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
///
/// Use instead:
/// ```python
/// {
///     "summary": "Does things.",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ManifestSummaryMultiline;

impl AlwaysFixableViolation for ManifestSummaryMultiline {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Summary in manifest file should be a one-line short description, found newline character"
            .to_string()
    }

    fn fix_title(&self) -> String {
        "Collapse the summary into a single line".to_string()
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

    let Some((key, value)) = manifest_item(dict, "summary") else {
        return;
    };
    let Expr::StringLiteral(ast::ExprStringLiteral { value: summary, .. }) = value else {
        return;
    };
    if !summary.to_str().contains('\n') {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(ManifestSummaryMultiline, key.range());

    // Rejoin the summary on single spaces: every newline becomes a space, and runs of
    // whitespace collapse so the fix never leaves two adjacent spaces (e.g. a triple-quoted
    // summary indented for readability collapses to plain prose).
    let single_line = summary
        .to_str()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let node = ast::StringLiteral {
        value: single_line.into_boxed_str(),
        flags: checker.default_string_flags().with_prefix({
            if summary.is_unicode() {
                StringLiteralPrefix::Unicode
            } else {
                StringLiteralPrefix::Empty
            }
        }),
        range: TextRange::default(),
        node_index: ruff_python_ast::AtomicNodeIndex::NONE,
    };
    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        checker.generator().expr(&node.into()),
        value.range(),
    )));
}
