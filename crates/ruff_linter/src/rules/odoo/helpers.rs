use std::path::Path;

use anyhow::{Context, Result};
use ruff_python_ast::{self as ast, Expr};
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

/// Returns the key and value expressions for `key` in a manifest dict literal, if present.
///
/// The key expression is what callers should report diagnostics on, matching pylint-odoo's
/// convention of pointing at the specific manifest key rather than the whole dict.
pub(crate) fn manifest_item<'a>(
    dict: &'a ast::ExprDict,
    key: &str,
) -> Option<(&'a Expr, &'a Expr)> {
    dict.items.iter().find_map(|item| {
        let key_expr = item.key.as_ref()?;
        let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = key_expr else {
            return None;
        };
        (value.to_str() == key).then_some((key_expr, &item.value))
    })
}

/// Returns the string value of `key` in a manifest dict literal, and the key expression to
/// report on, if `key` is present and its value is a plain string literal.
pub(crate) fn manifest_string_item<'a>(
    dict: &'a ast::ExprDict,
    key: &str,
) -> Option<(&'a Expr, &'a str)> {
    let (key_expr, value) = manifest_item(dict, key)?;
    let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = value else {
        return None;
    };
    Some((key_expr, value.to_str()))
}

/// Renders `expr` as a dotted name (e.g. `self.env.cr`) if it's a chain of attribute accesses
/// rooted at a plain name; returns `None` for anything else (calls, subscripts, etc.).
pub(crate) fn dotted_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(ast::ExprName { id, .. }) => Some(id.to_string()),
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            Some(format!("{}.{attr}", dotted_name(value)?))
        }
        _ => None,
    }
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
