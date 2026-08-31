use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_python_stdlib::identifiers::is_identifier;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::importer::ImportRequest;
use crate::line_width::LineWidthBuilder;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for the `_sql_constraints` attribute on an Odoo model class.
///
/// ## Why is this bad?
/// Odoo 19.0 dropped the attribute in favor of `models.Constraint` descriptors. Loading a
/// model that still carries it only logs `Model attribute '_sql_constraints' is no longer
/// supported, please define models.Constraint on the model.` and carries on, so the module
/// installs while every constraint it declared silently stops being created: the database
/// loses the uniqueness and check rules it used to enforce, and nothing fails until the data
/// they were guarding against shows up.
///
/// ## Example
/// ```python
/// class ResPartnerCategory(models.Model):
///     _name = "res.partner.category"
///
///     _sql_constraints = [
///         ("name_uniq", "unique (name)", "The name must be unique!"),
///     ]
/// ```
///
/// Use instead:
/// ```python
/// class ResPartnerCategory(models.Model):
///     _name = "res.partner.category"
///
///     _name_uniq = models.Constraint("unique (name)", "The name must be unique!")
/// ```
///
/// ## Options
/// - `lint.odoo.odoo-version`
///
/// ## Fix safety
/// The constraint keeps its identity in the database: both APIs name it
/// `{table}_{key}`, and the key is what the attribute is called minus its leading
/// underscore, so `("name_uniq", ...)` has to become `_name_uniq` for the database to see
/// the same constraint it already has. The fix is therefore a rename on the Python side
/// only, with no migration script to write.
///
/// It is marked as unsafe unless [`odoo-version`](../settings.md#lint_odoo_odoo-version) is
/// set to 19.0 or later, because `models.Constraint` does not exist before 19.0: on an older
/// Odoo the rewritten model raises `AttributeError` at import. With the version configured,
/// the rewrite is behavior-preserving and the fix is safe.
///
/// No fix is offered when the rewrite cannot be done by moving source around: a value that
/// is not a list of `(key, definition[, message])` tuples, a key that is not an identifier
/// or already starts with `_` (which Python would mangle), a key the class already binds, a
/// duplicated key, an entry spread over several lines, or a comment inside the assignment
/// that the rewrite would drop.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.33")]
pub(crate) struct DeprecatedSqlConstraints;

impl Violation for DeprecatedSqlConstraints {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Model attribute `_sql_constraints` is no longer supported since Odoo 19.0, \
         define `models.Constraint` attributes on the model instead"
            .to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Replace with `models.Constraint` attributes".to_string())
    }
}

/// The Odoo version that stopped reading `_sql_constraints`.
const SINCE: OdooVersion = OdooVersion::new(19, 0);

/// ODE9501
pub(crate) fn deprecated_sql_constraints(checker: &Checker, assign: &ast::StmtAssign) {
    // Up to 18.0 the attribute is *the* way to declare a SQL constraint, so reporting it
    // there would be reporting correct code.
    if !odoo_version_applies(checker, Some(SINCE), None) {
        return;
    }
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }
    let [Expr::Name(target)] = assign.targets.as_slice() else {
        return;
    };
    if target.id.as_str() != "_sql_constraints" {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(DeprecatedSqlConstraints, target.range());
    if let Some(fix) = constraint_attributes(checker, class_def, assign) {
        diagnostic.set_fix(fix);
    }
}

/// One rewritable `(key, definition[, message])` entry of an `_sql_constraints` list.
struct Constraint<'a> {
    /// The class attribute the entry becomes, leading underscore included: the key of
    /// `("name_uniq", ...)` names the attribute `_name_uniq`.
    attribute: String,
    /// The source of the arguments to hand to `models.Constraint`, verbatim, so that the
    /// original quoting and any `_(...)` wrapper survive the rewrite.
    arguments: Vec<&'a str>,
}

/// Builds the fix that replaces the whole `_sql_constraints` assignment with one
/// `models.Constraint` attribute per entry, or `None` when the entries are not something the
/// rewrite can move around without changing what the code says.
fn constraint_attributes(
    checker: &Checker,
    class_def: &ast::StmtClassDef,
    assign: &ast::StmtAssign,
) -> Option<Fix> {
    // The rewrite rebuilds the assignment from its sub-expressions, so a comment anywhere
    // inside it — typically one explaining a constraint — would be dropped.
    if checker.comment_ranges().intersects(assign.range()) {
        return None;
    }

    let locator = checker.locator();
    // Every generated attribute is written at the indentation of the assignment, which only
    // means anything if the assignment starts its own line: `x = 1; _sql_constraints = [...]`
    // would have its statements re-indented onto `x`'s line.
    let indent = locator.slice(TextRange::new(
        locator.line_start(assign.start()),
        assign.start(),
    ));
    if !indent.chars().all(char::is_whitespace) {
        return None;
    }

    let (Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. })) =
        assign.value.as_ref()
    else {
        return None;
    };
    // An empty list declares nothing, so there is no attribute to write in its place; the
    // assignment would have to be deleted outright, which is a different edit.
    if elts.is_empty() {
        return None;
    }

    let mut constraints: Vec<Constraint> = Vec::with_capacity(elts.len());
    for elt in elts {
        let (Expr::Tuple(ast::ExprTuple { elts: entry, .. })
        | Expr::List(ast::ExprList { elts: entry, .. })) = elt
        else {
            return None;
        };
        // `models.Constraint` takes the definition and, optionally, the message; the key
        // moves to the attribute name.
        let [Expr::StringLiteral(key), rest @ ..] = entry.as_slice() else {
            return None;
        };
        if rest.is_empty() || rest.len() > 2 {
            return None;
        }

        let key = key.value.to_str();
        // `TableObject.__set_name__` rejects a mangled name, and a key already starting with
        // `_` is exactly what produces one: `("_foo", ...)` would become `__foo`, which
        // Python rewrites to `_ClassName__foo` inside the class body.
        if key.starts_with('_') {
            return None;
        }
        let attribute = format!("_{key}");
        if !is_identifier(&attribute) {
            return None;
        }
        // Writing over something the class already binds would change what that name means,
        // and two entries sharing a key would leave only the last one standing.
        if class_binds(class_def, &attribute)
            || constraints
                .iter()
                .any(|constraint| constraint.attribute == attribute)
        {
            return None;
        }

        let mut arguments = Vec::with_capacity(rest.len());
        for argument in rest {
            let source = locator.slice(argument.range());
            // A value spread over several lines — an implicit concatenation, a backslash
            // continuation — cannot be spliced into the new call without re-indenting its
            // continuation lines, so leave the whole assignment to be rewritten by hand.
            if source.contains('\n') {
                return None;
            }
            arguments.push(source);
        }
        constraints.push(Constraint {
            attribute,
            arguments,
        });
    }

    let (import_edit, models) = checker
        .importer()
        .get_or_import_symbol(
            &ImportRequest::import_from("odoo", "models"),
            assign.start(),
            checker.semantic(),
        )
        .ok()?;

    let line_ending = checker.stylist().line_ending().as_str();
    let unit = checker.stylist().indentation().as_str();
    let tab_size = checker.settings().tab_size;
    let max_line_length = checker.settings().pycodestyle.max_line_length.value() as usize;

    let mut replacement = String::new();
    for (index, constraint) in constraints.iter().enumerate() {
        if index > 0 {
            replacement.push_str(line_ending);
            replacement.push_str(indent);
        }
        let Constraint {
            attribute,
            arguments,
        } = constraint;
        let one_line = format!(
            "{attribute} = {models}.Constraint({})",
            arguments.join(", ")
        );
        if LineWidthBuilder::new(tab_size)
            .add_str(indent)
            .add_str(&one_line)
            .get()
            <= max_line_length
        {
            replacement.push_str(&one_line);
            continue;
        }
        // Too long for one line: one argument per line, with a magic trailing comma so that
        // `ruff format` leaves the call expanded instead of collapsing it back over the limit.
        replacement.push_str(attribute);
        replacement.push_str(" = ");
        replacement.push_str(&models);
        replacement.push_str(".Constraint(");
        for argument in arguments {
            replacement.push_str(line_ending);
            replacement.push_str(indent);
            replacement.push_str(unit);
            replacement.push_str(argument);
            replacement.push(',');
        }
        replacement.push_str(line_ending);
        replacement.push_str(indent);
        replacement.push(')');
    }

    let edit = Edit::range_replacement(replacement, assign.range());
    // `models.Constraint` only exists from 19.0 on, so the rewrite is only known to preserve
    // behavior when the project says which Odoo it targets.
    if checker
        .settings()
        .odoo
        .odoo_version
        .is_some_and(|odoo_version| odoo_version >= SINCE)
    {
        Some(Fix::safe_edits(edit, [import_edit]))
    } else {
        Some(Fix::unsafe_edits(edit, [import_edit]))
    }
}

/// Returns `true` if the class body already binds `name`, whether as a plain or annotated
/// assignment or as a method.
fn class_binds(class_def: &ast::StmtClassDef, name: &str) -> bool {
    class_def.body.iter().any(|stmt| match stmt {
        ast::Stmt::Assign(assign) => assign
            .targets
            .iter()
            .any(|target| matches!(target, Expr::Name(target) if target.id == name)),
        ast::Stmt::AnnAssign(assign) => {
            matches!(assign.target.as_ref(), Expr::Name(target) if target.id == name)
        }
        ast::Stmt::FunctionDef(function_def) => function_def.name.as_str() == name,
        _ => false,
    })
}
