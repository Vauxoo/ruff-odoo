use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_trivia::CommentRanges;
use ruff_source_file::LineRanges;
use ruff_text_size::{TextLen, TextRange};

use crate::Locator;
use crate::checkers::ast::LintContext;
use crate::{AlwaysFixableViolation, Edit, Fix};

/// ## What it does
/// Checks for stray comments before the first line of code in a file.
///
/// ## Why is this bad?
/// A comment at the very top of a file that isn't a shebang, encoding declaration, or a
/// directive for a specific tool (pylint, flake8, noqa, isort, fmt, ...) is usually leftover
/// clutter (e.g. a commented-out license header from a template).
///
/// ## Example
/// ```python
/// # TODO: remove this file
/// import os
/// ```
///
/// Use instead:
/// ```python
/// import os
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct HeaderComments;

impl AlwaysFixableViolation for HeaderComments {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Use of header comments".to_string()
    }

    fn fix_title(&self) -> String {
        "Remove stray header comment".to_string()
    }
}

/// Comments matching one of these (as a substring) are left alone, regardless of position.
const VALID_HEADER_COMMENTS: &[&str] = &[
    "# !", // shebang
    "#!",  // shebang
    "coding:",
    "fixit:",
    "lint-ignore:",
    "lint-ignore=",
    "flake8:",
    "fmt:",
    "isort:",
    "noqa:",
    "nosec:",
    "oca-hooks:",
    "pylint:",
];

/// Returns `true` if every non-blank line in the file is a comment. The original Fixit check
/// this rule ports skipped comment-only files entirely, since deleting every stray comment
/// would otherwise leave the file empty.
fn file_is_comments_only(locator: &Locator, comment_ranges: &CommentRanges) -> bool {
    let mut last_end = locator.bom_start_offset();
    for comment_range in comment_ranges {
        let before_comment = locator.slice(TextRange::new(last_end, comment_range.start()));
        if !before_comment.trim().is_empty() {
            return false;
        }
        last_end = locator.full_line_range(comment_range.start()).end();
    }
    locator
        .slice(TextRange::new(last_end, locator.contents().text_len()))
        .trim()
        .is_empty()
}

/// ODC8501
pub(crate) fn header_comments(
    context: &LintContext,
    locator: &Locator,
    comment_ranges: &CommentRanges,
) {
    if file_is_comments_only(locator, comment_ranges) {
        return;
    }

    let mut last_end = locator.bom_start_offset();

    for comment_range in comment_ranges {
        let line_range = locator.full_line_range(comment_range.start());
        let before_comment = locator.slice(TextRange::new(last_end, comment_range.start()));
        if !before_comment.trim().is_empty() {
            // Reached a non-blank, non-comment line: no longer in the file's leading block.
            break;
        }
        last_end = line_range.end();

        let text = locator.slice(comment_range);
        if VALID_HEADER_COMMENTS
            .iter()
            .any(|valid| text.contains(valid))
        {
            continue;
        }

        if let Some(mut diagnostic) =
            context.report_diagnostic_if_enabled(HeaderComments, comment_range)
        {
            diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(line_range)));
        }
    }
}
