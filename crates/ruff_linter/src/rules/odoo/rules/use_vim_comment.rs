use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_source_file::LineRanges;
use ruff_text_size::TextRange;

use crate::Locator;
use crate::checkers::ast::LintContext;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for vim modeline comments (e.g. `# vim: fileencoding=utf-8`).
///
/// ## Why is this bad?
/// Vim modelines configure the editor on a per-file basis, which is better
/// handled by a local (untracked) `.vimrc` or `.lvimrc`, so it doesn't leak
/// one contributor's editor preferences into the shared codebase.
///
/// ## Example
/// ```python
/// # vim: fileencoding=utf-8
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct UseVimComment;

impl Violation for UseVimComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Use of vim comment".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Remove vim comment".to_string())
    }
}

/// Returns `true` if `text` (the source of a comment token, `#`-prefixed) is a vim modeline.
fn is_vim_comment(text: &str) -> bool {
    text.trim_matches(|c: char| c == '#' || c == ' ')
        .to_lowercase()
        .starts_with("vim:")
}

/// ODW8202
pub(crate) fn use_vim_comment(context: &LintContext, locator: &Locator, comment_range: TextRange) {
    if !is_vim_comment(locator.slice(comment_range)) {
        return;
    }

    let Some(mut diagnostic) = context.report_diagnostic_if_enabled(UseVimComment, comment_range)
    else {
        return;
    };

    // Only offer a fix when the comment is the sole content of its line; an inline trailing
    // comment (e.g. `x = 1  # vim: set ts=4`) is left for the user to clean up by hand.
    let line_range = locator.full_line_range(comment_range.start());
    let before_comment = locator.slice(TextRange::new(line_range.start(), comment_range.start()));
    if before_comment.trim().is_empty() {
        diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(line_range)));
    }
}
