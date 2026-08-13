use std::path::Path;

use anyhow::{Context, Result};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_trivia::{SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextLen, TextRange};

use crate::Edit;
use crate::checkers::ast::Checker;
use crate::line_width::LineWidthBuilder;

/// Renders `content` as string-literal pieces for a parenthesized implicit concatenation.
///
/// Pieces split only right after spaces, so concatenating them reproduces `content`
/// exactly (the word separator stays at the end of each non-final piece). Each piece fits
/// within `max_line_length` when written at `indent`, whenever a space to break at exists;
/// a single word longer than the limit stays on its own overlong line.
pub(crate) fn wrap_string_literal(
    checker: &Checker,
    flags: ast::StringLiteralFlags,
    content: &str,
    indent: &str,
    max_line_length: usize,
) -> Vec<String> {
    let tab_size = checker.settings().tab_size;
    let render = |chunk: &str| {
        checker.generator().expr(
            &ast::StringLiteral {
                value: chunk.into(),
                flags,
                range: TextRange::default(),
                node_index: ast::AtomicNodeIndex::NONE,
            }
            .into(),
        )
    };
    let mut pieces = Vec::new();
    let mut current = String::new();
    for word in content.split_inclusive(' ') {
        if !current.is_empty() {
            let width = LineWidthBuilder::new(tab_size)
                .add_str(indent)
                .add_str(&render(&format!("{current}{word}")))
                .get();
            if width > max_line_length {
                pieces.push(render(&current));
                current.clear();
            }
        }
        current.push_str(word);
    }
    pieces.push(render(&current));
    pieces
}

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

/// Anchor range for diagnostics about something missing from the manifest (a required key,
/// a README next to it): the `"name"` key when present, else the opening `{`. Reporting on
/// the whole dict would span every line of the manifest in editors.
pub(crate) fn manifest_anchor_range(dict: &ast::ExprDict) -> TextRange {
    manifest_item(dict, "name").map_or_else(
        || TextRange::at(dict.start(), '{'.text_len()),
        |(key, _value)| key.range(),
    )
}

const ODOO_MODEL_BASES: &[&str] = &["Model", "TransientModel", "AbstractModel"];

/// Returns `true` if `class_def`'s bases include (by unqualified name) an Odoo model base,
/// e.g. `models.Model` or `Model`.
pub(crate) fn is_odoo_model_class(class_def: &ast::StmtClassDef) -> bool {
    let Some(arguments) = class_def.arguments.as_deref() else {
        return false;
    };
    arguments.args.iter().any(|base| {
        let name = match base {
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str(),
            Expr::Name(ast::ExprName { id, .. }) => id.as_str(),
            _ => return false,
        };
        ODOO_MODEL_BASES.contains(&name)
    })
}

/// Returns the field type (e.g. `"Many2one"`) if `func` is an access on `fields`, as in
/// `fields.Many2one(...)`.
pub(crate) fn odoo_field_type(func: &Expr) -> Option<&str> {
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func else {
        return None;
    };
    matches!(value.as_ref(), Expr::Name(name) if name.id == "fields").then_some(attr.as_str())
}

/// Returns `true` if the class body defines a function named `name`.
pub(crate) fn class_defines_method(class_def: &ast::StmtClassDef, name: &str) -> bool {
    class_def.body.iter().any(|stmt| {
        matches!(stmt, ast::Stmt::FunctionDef(function_def) if function_def.name.as_str() == name)
    })
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

/// Manifest keys whose values are lists of module data file paths.
pub(crate) const MANIFEST_DATA_KEYS: &[&str] =
    &["data", "demo", "demo_xml", "init_xml", "test", "update_xml"];

/// Generate an [`Edit`] to remove `item` (a key-value pair) from `dict`, including its
/// surrounding comma, leaving the rest of the dictionary display intact.
pub(crate) fn remove_dict_item(
    dict: &ast::ExprDict,
    item: &ast::DictItem,
    source: &str,
) -> Result<Edit> {
    let ranges: Vec<_> = dict.items.iter().map(Ranged::range).collect();
    remove_sequence_element(&ranges, item.range(), source)
}

/// Generate an [`Edit`] to remove `element` from a list display, including its surrounding
/// comma, leaving the rest of the list intact.
pub(crate) fn remove_list_element(
    list: &ast::ExprList,
    element: &Expr,
    source: &str,
) -> Result<Edit> {
    let ranges: Vec<_> = list.elts.iter().map(Ranged::range).collect();
    remove_sequence_element(&ranges, element.range(), source)
}

/// Generate an [`Edit`] to remove the element spanning `target` from the comma-separated
/// sequence whose element ranges are `ranges`, including its surrounding comma.
fn remove_sequence_element(ranges: &[TextRange], target: TextRange, source: &str) -> Result<Edit> {
    let (before, after): (Vec<_>, Vec<_>) = ranges
        .iter()
        .copied()
        .filter(|range| *range != target)
        .partition(|range| range.start() < target.start());

    if !after.is_empty() {
        // The element is not the last one, so delete from its start to the start of the next
        // non-trivia token following its trailing comma.
        let mut tokenizer = SimpleTokenizer::starts_at(target.end(), source);
        tokenizer
            .find(|token| token.kind == SimpleTokenKind::Comma)
            .context("Unable to find trailing comma")?;
        let next = tokenizer
            .find(|token| {
                token.kind != SimpleTokenKind::Whitespace && token.kind != SimpleTokenKind::Newline
            })
            .context("Unable to find next token")?;
        Ok(Edit::deletion(target.start(), next.start()))
    } else if let Some(previous) = before.iter().map(Ranged::end).max() {
        // The element is the last one, so delete from the start of the preceding comma to
        // the end of the element.
        let mut tokenizer = SimpleTokenizer::starts_at(previous, source);
        let comma = tokenizer
            .find(|token| token.kind == SimpleTokenKind::Comma)
            .context("Unable to find trailing comma")?;
        Ok(Edit::deletion(comma.start(), target.end()))
    } else {
        // The element is the only one in the sequence. Displays allow a trailing comma
        // after the last element, so remove that too if present.
        let mut tokenizer = SimpleTokenizer::starts_at(target.end(), source);
        let end = tokenizer
            .find(|token| {
                token.kind != SimpleTokenKind::Whitespace && token.kind != SimpleTokenKind::Newline
            })
            .filter(|token| token.kind == SimpleTokenKind::Comma)
            .map_or(target.end(), |token| token.end());
        Ok(Edit::deletion(target.start(), end))
    }
}
