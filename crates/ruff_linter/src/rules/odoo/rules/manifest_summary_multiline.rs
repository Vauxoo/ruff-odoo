use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, str_prefix::StringLiteralPrefix};
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::fix::edits::fits;
use crate::rules::odoo::helpers::{is_manifest_file, manifest_item, wrap_string_literal};
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
///
/// When the collapsed summary would exceed the configured line length, the fix wraps it
/// into a parenthesized implicit string concatenation instead:
/// ```python
/// {
///     "summary": (
///         "A collapsed summary too long for one line, wrapped into pieces "
///         "that each fit within the line length."
///     ),
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
    let flags = checker.default_string_flags().with_prefix({
        if summary.is_unicode() {
            StringLiteralPrefix::Unicode
        } else {
            StringLiteralPrefix::Empty
        }
    });
    let single_repr = checker.generator().expr(
        &ast::StringLiteral {
            value: single_line.clone().into_boxed_str(),
            flags,
            range: TextRange::default(),
            node_index: ruff_python_ast::AtomicNodeIndex::NONE,
        }
        .into(),
    );
    let locator = checker.locator();
    let line_start = locator.line_start(key.range().start());
    let indent = locator.slice(TextRange::new(line_start, key.range().start()));

    // Measure with the trailing comma included so a fix landing exactly on the limit
    // doesn't overflow once applied. Wrapping also needs the key to start its own line
    // (pure-whitespace indentation); otherwise fall back to the single-line fix.
    if fits(
        &format!("{single_repr},"),
        value.into(),
        locator,
        checker.settings().pycodestyle.max_line_length,
        checker.settings().tab_size,
    ) || !indent.chars().all(char::is_whitespace)
    {
        diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
            single_repr,
            value.range(),
        )));
        return;
    }

    // The collapsed summary exceeds the line length: wrap it into a parenthesized implicit
    // concatenation, one piece per line, keeping the word separator at the end of each piece
    // so the concatenated value stays identical to the single-line form.
    let max_line_length = checker.settings().pycodestyle.max_line_length.value() as usize;
    let inner_indent = format!("{}{}", indent, checker.stylist().indentation().as_str());
    let pieces = wrap_string_literal(checker, flags, &single_line, &inner_indent, max_line_length);

    let line_ending = checker.stylist().line_ending().as_str();
    let mut wrapped = String::from("(");
    for piece in &pieces {
        wrapped.push_str(line_ending);
        wrapped.push_str(&inner_indent);
        wrapped.push_str(piece);
    }
    wrapped.push_str(line_ending);
    wrapped.push_str(indent);
    wrapped.push(')');

    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        wrapped,
        value.range(),
    )));
}
