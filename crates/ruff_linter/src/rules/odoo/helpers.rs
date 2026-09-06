use std::path::Path;

use anyhow::{Context, Result};
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::SemanticModel;
use ruff_python_trivia::{SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextLen, TextRange};

use crate::Edit;
use crate::checkers::ast::Checker;
use crate::line_width::LineWidthBuilder;
use crate::rules::odoo::settings::OdooVersion;

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

/// Returns `true` if the dict literal currently being visited is the manifest's top-level
/// dict: `path` is a manifest file, we're at module scope, and the dict has no parent
/// expression (it isn't nested inside another dict/list/call, e.g. the `"assets"` sub-dict).
/// Without the last check, rules would also fire on every nested dict literal in the
/// manifest, since nested literals stay in module scope too (only
/// `class`/`def`/`lambda`/comprehensions introduce a new scope).
///
/// Must be called while the checker is visiting the candidate `Expr::Dict` node.
pub(crate) fn is_manifest_root_dict(checker: &Checker, dict: &ast::ExprDict, path: &Path) -> bool {
    is_manifest_file(path)
        && checker.semantic().current_scope().kind.is_module()
        && checker.semantic().current_expression_parent().is_none()
        && is_dict_literal_evaluable(dict)
}

/// Returns `true` if every key and value in `dict` is something Python's `ast.literal_eval`
/// would accept (recursively): a constant, or a list/tuple/set/dict built purely from such
/// constants. pylint-odoo parses `__manifest__.py` with `ast.literal_eval` and silently skips
/// *every* manifest-content check for a file where that raises `ValueError` — e.g. a value
/// using `or`/`and`, a function call, or a name reference instead of a literal (`{"key": "" or
/// ""}`). Manifest-content rules must reproduce that same skip, or they read expressions
/// pylint-odoo's own checker never actually evaluates.
fn is_dict_literal_evaluable(dict: &ast::ExprDict) -> bool {
    dict.items.iter().all(|item| {
        item.key.as_ref().is_some_and(is_literal_evaluable_expr)
            && is_literal_evaluable_expr(&item.value)
    })
}

fn is_literal_evaluable_expr(expr: &Expr) -> bool {
    match expr {
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_) => true,
        // `ast.literal_eval` allows a leading `+`/`-` on a numeric literal.
        Expr::UnaryOp(ast::ExprUnaryOp { op, operand, .. }) => {
            matches!(op, ast::UnaryOp::USub | ast::UnaryOp::UAdd)
                && matches!(operand.as_ref(), Expr::NumberLiteral(_))
        }
        Expr::List(ast::ExprList { elts, .. })
        | Expr::Tuple(ast::ExprTuple { elts, .. })
        | Expr::Set(ast::ExprSet { elts, .. }) => elts.iter().all(is_literal_evaluable_expr),
        Expr::Dict(dict) => is_dict_literal_evaluable(dict),
        _ => false,
    }
}

/// Returns `true` if `class_def`'s bases include an Odoo model base (`models.Model`,
/// `TransientModel`, ...) resolved through imports, e.g. `from odoo import models` or
/// `from odoo.models import Model`. A bare `Model` base from an unrelated `models` module
/// doesn't count.
pub(crate) fn is_odoo_model_class(semantic: &SemanticModel, class_def: &ast::StmtClassDef) -> bool {
    let Some(arguments) = class_def.arguments.as_deref() else {
        return false;
    };
    arguments.args.iter().any(|base| {
        matches!(
            semantic
                .resolve_qualified_name(base)
                .as_ref()
                .map(QualifiedName::segments),
            Some([
                "odoo",
                "models",
                "Model" | "TransientModel" | "AbstractModel"
            ])
        )
    })
}

/// Returns `true` if `class_def` inherits from something that is not a Python builtin.
///
/// A far looser test than [`is_odoo_model_class`], and deliberately so: an Odoo addon
/// defines models, controllers, wizards, report parsers and mixins, and only the first of
/// those inherits `models.Model` by that literal name. A controller inherits
/// `http.Controller`, a class in a large addon inherits another class of the addon, and both
/// still hold Odoo code calling Odoo methods. What this excludes is the rest of a Python
/// file: `class Foo:`, `class Bar(object):` and `class MyError(Exception):` are Python, not
/// Odoo, and a method name matched inside one says nothing about the ORM.
pub(crate) fn inherits_non_builtin(
    semantic: &SemanticModel,
    class_def: &ast::StmtClassDef,
) -> bool {
    class_def.bases().iter().any(|base| {
        // A base resolving to nothing at all -- imported from a module the checker did not
        // read, which is every Odoo import -- counts: unresolvable is not builtin.
        semantic.resolve_builtin_symbol(base).is_none()
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

/// Returns `true` if the class body declares `model` through `_name` or `_inherit`.
///
/// Both spellings count, and `_inherit` is read as either a single name or a list of them,
/// because Odoo merges the class into every model it names.
pub(crate) fn class_declares_model(class_def: &ast::StmtClassDef, model: &str) -> bool {
    class_def.body.iter().any(|stmt| {
        let ast::Stmt::Assign(assign) = stmt else {
            return false;
        };
        if !assign.targets.iter().any(
            |target| matches!(target, Expr::Name(name) if name.id == "_name" || name.id == "_inherit"),
        ) {
            return false;
        }
        match assign.value.as_ref() {
            Expr::StringLiteral(literal) => literal.value.to_str() == model,
            Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. }) => {
                elts.iter().any(
                    |elt| matches!(elt, Expr::StringLiteral(literal) if literal.value.to_str() == model),
                )
            }
            _ => false,
        }
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

/// Returns `true` if a rule scoped to Odoo versions `min..=max` (either bound optional)
/// should apply given `checker`'s configured `odoo-version`.
///
/// Mirrors pylint-odoo's `checks_maxmin_odoo_version`: when no `odoo-version` is configured,
/// version-scoped rules stay enabled unconditionally (so behavior doesn't regress for users
/// who haven't opted into the setting).
pub(crate) fn odoo_version_applies(
    checker: &Checker,
    min: Option<OdooVersion>,
    max: Option<OdooVersion>,
) -> bool {
    let Some(odoo_version) = checker.settings().odoo.odoo_version else {
        return true;
    };
    min.is_none_or(|min| odoo_version >= min) && max.is_none_or(|max| odoo_version <= max)
}

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

/// The expressions that denote a database cursor, as pylint-odoo's `cursor-expr` defaults.
///
/// Shared by `invalid-commit` (`ODE8102`) and `sql-injection` (`ODE8103`): both answer "is this
/// a cursor?", both read the same `cursor-expr` setting, and a default written twice is a
/// default that eventually disagrees with itself.
pub(crate) const CURSOR_EXPRS: &[&str] = &["cr", "self._cr", "self.cr", "self.env.cr"];
