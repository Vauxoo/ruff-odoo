use std::path::Path;

use anyhow::{Context, Result};
use ruff_python_ast as ast;
use ruff_python_trivia::{SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::Ranged;

use crate::Edit;

/// Returns `true` if `path` is an Odoo module manifest file (`__manifest__.py`, or the
/// legacy `__openerp__.py` name).
pub(crate) fn is_manifest_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("__manifest__.py" | "__openerp__.py")
    )
}

/// Generate an [`Edit`] to remove `item` (a key-value pair) from `dict`, including its
/// surrounding comma, leaving the rest of the dictionary display intact.
pub(crate) fn remove_dict_item(
    dict: &ast::ExprDict,
    item: &ast::DictItem,
    source: &str,
) -> Result<Edit> {
    let (before, after): (Vec<_>, Vec<_>) = dict
        .items
        .iter()
        .map(Ranged::range)
        .filter(|range| *range != item.range())
        .partition(|range| range.start() < item.start());

    if !after.is_empty() {
        // The item is not the last one, so delete from its start to the start of the next
        // non-trivia token following its trailing comma.
        let mut tokenizer = SimpleTokenizer::starts_at(item.end(), source);
        tokenizer
            .find(|token| token.kind == SimpleTokenKind::Comma)
            .context("Unable to find trailing comma")?;
        let next = tokenizer
            .find(|token| {
                token.kind != SimpleTokenKind::Whitespace && token.kind != SimpleTokenKind::Newline
            })
            .context("Unable to find next token")?;
        Ok(Edit::deletion(item.start(), next.start()))
    } else if let Some(previous) = before.iter().map(Ranged::end).max() {
        // The item is the last one, so delete from the start of the preceding comma to the
        // end of the item.
        let mut tokenizer = SimpleTokenizer::starts_at(previous, source);
        let comma = tokenizer
            .find(|token| token.kind == SimpleTokenKind::Comma)
            .context("Unable to find trailing comma")?;
        Ok(Edit::deletion(comma.start(), item.end()))
    } else {
        // The item is the only one in the dictionary. Dict literals allow a trailing comma
        // after the last item, so remove that too if present.
        let mut tokenizer = SimpleTokenizer::starts_at(item.end(), source);
        let end = tokenizer
            .find(|token| {
                token.kind != SimpleTokenKind::Whitespace && token.kind != SimpleTokenKind::Newline
            })
            .filter(|token| token.kind == SimpleTokenKind::Comma)
            .map_or(item.end(), |token| token.end());
        Ok(Edit::deletion(item.start(), end))
    }
}
