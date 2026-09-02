use rustc_hash::FxHashMap;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_source_file::SourceRow;
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_field_type};

/// ## What it does
/// Checks for a field declared more than once in the body of the same Odoo model class.
///
/// ## Why is this bad?
/// A class body is executed top to bottom, so only the last assignment to a name survives in
/// the class `__dict__` the ORM reads. Every earlier declaration of the same field is dead:
/// its comodel, its `relation` table, its `compute` and its label never reach the registry,
/// and nothing reports that they were dropped.
///
/// The two declarations rarely agree. When they don't, the field that ends up in the
/// database is the last one, which is the opposite of what a reader scanning the class from
/// the top concludes. A `Many2many` whose dead declaration named a `relation` is the sharp
/// case: that table is never created, while the code naming it still reads as if it were.
///
/// Declaring the same field again in a *different* file or module is not this. That is Odoo
/// inheritance -- a module extending a model and overriding one of its fields -- and it is
/// the supported way to change a field. Only a name bound twice inside one class body is
/// reported, and a declaration is recognised by its `fields.<Type>(...)` call, the spelling
/// Odoo models use.
///
/// ## Example
/// ```python
/// class ResPartner(models.Model):
///     _inherit = "res.partner"
///
///     category_ids = fields.Many2many(
///         "res.partner.category.report",
///         relation="res_partner_res_partner_category_report_rel",
///     )
///     category_ids = fields.Many2many(
///         "res.partner.category",
///         relation="res_partner_res_partner_category_rel",
///     )
/// ```
///
/// Use instead:
/// ```python
/// class ResPartner(models.Model):
///     _inherit = "res.partner"
///
///     category_ids = fields.Many2many(
///         "res.partner.category",
///         relation="res_partner_res_partner_category_rel",
///     )
/// ```
///
/// One diagnostic is reported per duplicated field, anchored on its first declaration and
/// listing every other one, so a field declared three times reads as a single finding rather
/// than as two. Removing the declarations that have no effect is the mechanically correct
/// edit: the last one is what the ORM already read and what the database follows, so dropping
/// the others cannot change how the code runs. No fix is offered even so. The duplicate is
/// normally an accident, which means the surviving declaration is not necessarily the one
/// anybody chose, and applying the edit automatically would settle that question without
/// anybody looking at it.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.33")]
pub(crate) struct DuplicateFieldDeclaration {
    name: String,
    row: SourceRow,
}

impl Violation for DuplicateFieldDeclaration {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DuplicateFieldDeclaration { name, row } = self;
        format!(
            "Field `{name}` is declared multiple times; only the declaration on {row} takes \
             effect"
        )
    }
}

/// A name bound by a statement of a class body, and whether the value bound to it is an Odoo
/// field. Which binding comes last decides what the ORM reads, so a method or a plain value
/// taking a field's name has to be seen as well as the field declarations themselves.
#[derive(Clone, Copy)]
struct ClassBinding<'a> {
    name: &'a str,
    range: TextRange,
    is_field: bool,
}

/// Returns `true` if `value` is a `fields.<Type>(...)` call.
fn is_field_call(value: &Expr) -> bool {
    matches!(value, Expr::Call(call) if odoo_field_type(&call.func).is_some())
}

/// Appends the names `target` binds to `value`, whose field-ness is `is_field`.
fn collect_target<'a>(target: &'a Expr, is_field: bool, bindings: &mut Vec<ClassBinding<'a>>) {
    match target {
        Expr::Name(name) => bindings.push(ClassBinding {
            name: name.id.as_str(),
            range: name.range(),
            is_field,
        }),
        // Unpacking binds each element to a piece of the value, never to the value itself, so
        // `first_id, second_id = ...` declares no field however its right-hand side reads.
        Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
            for element in elts {
                collect_target(element, false, bindings);
            }
        }
        _ => {}
    }
}

/// Appends the class attributes `stmt` binds when the class body runs.
fn collect_bindings<'a>(stmt: &'a Stmt, bindings: &mut Vec<ClassBinding<'a>>) {
    match stmt {
        Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
            let is_field = is_field_call(value);
            for target in targets {
                collect_target(target, is_field, bindings);
            }
        }
        // A bare annotation (`amount: float`) binds nothing when the class body runs, so it
        // leaves the attribute the ORM reads untouched.
        Stmt::AnnAssign(ast::StmtAnnAssign {
            target,
            value: Some(value),
            ..
        }) => collect_target(target, is_field_call(value), bindings),
        // A method or a nested class binds its name too, and takes a field's place when it
        // reuses one.
        Stmt::FunctionDef(ast::StmtFunctionDef { name, .. })
        | Stmt::ClassDef(ast::StmtClassDef { name, .. }) => bindings.push(ClassBinding {
            name: name.as_str(),
            range: name.range(),
            is_field: false,
        }),
        _ => {}
    }
}

/// ODW9503
pub(crate) fn duplicate_field_declaration(checker: &Checker, class_def: &ast::StmtClassDef) {
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }

    // Only statements directly in the class body count: those are the ones that bind class
    // attributes, in the order the class body runs them.
    let mut bindings: Vec<ClassBinding> = Vec::new();
    for stmt in &class_def.body {
        collect_bindings(stmt, &mut bindings);
    }

    let mut by_name: FxHashMap<&str, Vec<ClassBinding>> = FxHashMap::default();
    // Field names in the order they are first bound, so the diagnostics of a class do not
    // depend on the iteration order of the map.
    let mut names: Vec<&str> = Vec::new();
    for binding in bindings {
        let entry = by_name.entry(binding.name).or_default();
        if entry.is_empty() {
            names.push(binding.name);
        }
        entry.push(binding);
    }

    for name in names {
        let Some([earlier @ .., surviving]) = by_name.get(name).map(Vec::as_slice) else {
            continue;
        };
        // Only the last binding is left in the class `__dict__`. When it is not a field, the
        // class ends up declaring no field under this name, which is a different problem and
        // not one this rule can describe.
        if !surviving.is_field {
            continue;
        }
        let mut discarded = earlier.iter().filter(|binding| binding.is_field);
        let Some(first) = discarded.next() else {
            continue;
        };
        let mut diagnostic = checker.report_diagnostic(
            DuplicateFieldDeclaration {
                name: name.to_string(),
                row: checker.compute_source_row(surviving.range.start()),
            },
            first.range,
        );
        diagnostic.set_primary_annotation_message("this declaration has no effect");
        for binding in discarded {
            diagnostic.secondary_annotation("also has no effect", binding.range);
        }
        diagnostic.secondary_annotation("this declaration takes effect", surviving.range);
        diagnostic.help(format_args!(
            "Remove all but the last declaration of `{name}`"
        ));
    }
}
