use std::path::Path;

use anyhow::{Context, Result};
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::{Imported, SemanticModel};
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

/// Returns `true` if the class body assigns `_name` or `_inherit` at its root.
///
/// One of the two is what makes a class a model: Odoo's registry keys a model by `_name`,
/// and a class without it extends the models its `_inherit` lists. A class that declares
/// neither adds nothing to the registry, whatever it inherits from.
///
/// `_inherits` is deliberately not accepted on its own. It is delegation, not definition —
/// a class using it still has to name itself through `_name`, so it is already covered.
pub(crate) fn class_declares_model_attribute(class_def: &ast::StmtClassDef) -> bool {
    class_def.body.iter().any(|stmt| {
        let targets: &[Expr] = match stmt {
            ast::Stmt::Assign(assign) => &assign.targets,
            ast::Stmt::AnnAssign(assign) => std::slice::from_ref(&assign.target),
            _ => return false,
        };
        targets.iter().any(
            |target| matches!(target, Expr::Name(name) if name.id == "_name" || name.id == "_inherit"),
        )
    })
}

/// Returns `true` if `class_def` is an Odoo model, which takes **both** halves:
///
/// 1. a base resolving to an Odoo model base through imports, e.g. `from odoo import
///    models` or `from odoo.models import Model` — a bare `Model` base from an unrelated
///    `models` module doesn't count; and
/// 2. a `_name` or `_inherit` assignment in the class body.
///
/// The model bases are the ones `odoo/models/__init__.py` exports: `Model`,
/// `TransientModel`, `AbstractModel` (an alias of `BaseModel`), `BaseModel` itself, and
/// `CachedModel`. `BaseModel` is what Odoo subclasses for a report over a SQL view, and
/// `CachedModel` arrived in 20.0 for the registry-cached models — `res.company`,
/// `res.currency`, `res.lang`, `res.country`, `website`. Both are accepted on every
/// version: nothing else in the ecosystem is called `models.CachedModel`, so accepting it
/// against an older Odoo cannot produce a false positive, and gating it on the version
/// would mean threading `odoo-version` into a helper that otherwise never needs it.
///
/// `MetaModel` is exported too but is the metaclass, not a base. `Constraint` and the
/// other table objects are exported from the same module and are not models: they define
/// no `_name`/`_inherit`, so the attribute half already rejects them.
///
/// Requiring the base alone was too loose. A class inheriting `models.Model` without
/// declaring either attribute defines no model — it is a base class other model classes
/// import, a scaffold, or dead code — and rules about fields, ORM methods and `self`
/// recordsets have nothing to say about it.
///
/// Requiring the attribute alone would be too loose in the other direction: OCA's
/// `component` framework reuses `_name` and `_inherit` to name components, which are not
/// ORM records, so `class Foo(Component): _inherit = "base"` would be reported as a model.
///
/// Both halves together are also enough. Odoo models are not built by subclassing another
/// model class in Python — a model extends another through `_inherit`, not through a base —
/// so there is no chain to follow, and the direct base is the whole answer.
pub(crate) fn is_odoo_model_class(semantic: &SemanticModel, class_def: &ast::StmtClassDef) -> bool {
    if !class_declares_model_attribute(class_def) {
        return false;
    }
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
                "Model" | "TransientModel" | "AbstractModel" | "BaseModel" | "CachedModel"
            ])
        )
    })
}

/// Directory names Odoo itself requires, whose contents are never controllers.
///
/// These are not a naming convention: the test loader collects from a directory literally
/// called `tests`, and the migration runner looks for `migrations/<version>/`. An addon
/// cannot rename them and keep working, so reading them is reading structure, not guessing.
/// `models/`, `controllers/` and `wizard/` are the opposite — pure convention, which is why
/// nothing here consults them.
const ODOO_STRUCTURAL_NON_CODE_DIRS: [&str; 2] = ["tests", "migrations"];

/// Returns `true` if `path` sits under a `tests/` or `migrations/` directory.
fn in_structural_non_code_dir(path: &Path) -> bool {
    path.parent().is_some_and(|parent| {
        parent.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| ODOO_STRUCTURAL_NON_CODE_DIRS.contains(&name))
        })
    })
}

/// Returns `true` if the file imports `odoo.http`, in any of its three spellings:
/// `from odoo import http`, `from odoo.http import ...`, and `import odoo.http`. All three
/// bind something whose qualified name starts `odoo.http`, so one pattern covers them.
fn file_imports_odoo_http(semantic: &SemanticModel) -> bool {
    semantic.global_scope().binding_ids().any(|binding_id| {
        semantic
            .binding(binding_id)
            .as_any_import()
            .is_some_and(|import| {
                matches!(import.qualified_name().segments(), ["odoo", "http", ..])
            })
    })
}

/// Returns `true` if `base` is a Python builtin or a test case, neither of which an Odoo
/// controller ever derives from.
fn is_builtin_or_test_base(semantic: &SemanticModel, base: &Expr) -> bool {
    if semantic.resolve_builtin_symbol(base).is_some() {
        return true;
    }
    matches!(
        semantic
            .resolve_qualified_name(base)
            .as_ref()
            .map(QualifiedName::segments),
        Some(["odoo", "tests", ..] | ["unittest" | "enum" | "abc", ..])
    )
}

/// Returns `true` if `class_def` is an Odoo HTTP controller.
///
/// Deriving from `odoo.http.Controller` by that literal name identifies barely a quarter of
/// them. Measured over the 12,493 addons under `~/odoo`, of the 3,964 classes living in a
/// `controllers/` directory only 1,072 name that base: 2,779 derive from another addon's
/// controller through `odoo.addons.*`, whose own base the linter cannot see, because
/// resolving a name never opens the module it came from. Odoo core does this itself, in
/// `auth_signup/controllers/main.py`.
///
/// So the test is not what the class derives from, but what the file it lives in imports.
/// A file importing `odoo.http` is a file doing HTTP work, and every controller needs it:
/// for `route`, for `request`, or for `Controller` itself. That takes recognition from 27%
/// to 93.6%, and the 250 classes still missed are ones whose file imports nothing from
/// `odoo.http` because everything they use came down from the inherited controller.
///
/// The breadth costs precision, since a file holds more than the class of interest, so four
/// exclusions narrow it back, each measured:
///
/// - **not a model.** Model files import `odoo.http` too. Costs 2 classes.
/// - **not baseless.** `class Foo:` in such a file is a data structure, as `Store` and
///   `StoreVersion` are in `mail/tools/discuss.py`. Removes 97.
/// - **no builtin or test-case base.** `class Foo(Exception)` and `class T(HttpCase)` are
///   not controllers. Removes 325.
/// - **not under `tests/` or `migrations/`.** Those directories import `odoo.http` in order
///   to exercise it. `tests/` holds 21,720 classes of which 7 carry any controller
///   evidence, and 4 of those 7 only appear to because they inherit a test helper that
///   happens to live in a `controllers` package; the remaining 3 are `CTRLFake`-style
///   stubs. `migrations/` holds 3 classes in the whole corpus and none is a controller.
///
/// What remains is 92.6% of controllers, and what it still over-reaches on is roughly 200
/// classes: API clients, `IoT` drivers and OCA components sharing a file with HTTP code.
///
/// The file's directory is never consulted beyond those two structural names. A controller
/// in a root-level `controllers.py`, as `auth_password_policy_portal` has, is found the
/// same way as one in `controllers/`.
pub(crate) fn is_odoo_controller_class(
    semantic: &SemanticModel,
    class_def: &ast::StmtClassDef,
    path: &Path,
) -> bool {
    if in_structural_non_code_dir(path) {
        return false;
    }
    if !file_imports_odoo_http(semantic) {
        return false;
    }
    if is_odoo_model_class(semantic, class_def) {
        return false;
    }
    let Some(arguments) = class_def.arguments.as_deref() else {
        return false;
    };
    !arguments.args.is_empty()
        && !arguments
            .args
            .iter()
            .any(|base| is_builtin_or_test_base(semantic, base))
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
