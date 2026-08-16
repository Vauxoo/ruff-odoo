use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{dotted_name, is_odoo_model_class, odoo_field_type};

/// ## What it does
/// Checks for `self.write(...)` calls inside compute methods.
///
/// ## Why is this bad?
/// Writing from a compute method triggers the full write path (constraints, recomputes,
/// audit fields) and can recurse; assigning via `update` (or plain field assignment) is the
/// supported way to set values from a compute.
///
/// ## Example
/// ```python
/// def _compute_total(self):
///     self.write({"total": 10})
/// ```
///
/// Use instead:
/// ```python
/// def _compute_total(self):
///     for record in self:
///         record.update({"total": 10})
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct NoWriteInCompute;

impl Violation for NoWriteInCompute {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Compute method calling `write`. Use `update` instead.".to_string()
    }
}

/// Root receivers (besides bare `self`) whose `.write(...)` call is flagged, mirroring
/// pylint-odoo's `check_no_write_compute`: every `self.<method>` that returns a recordset,
/// plus any `self.with_*` (e.g. `self.with_context`, `self.with_company`).
const RECORDSET_ROOT_METHODS: &[&str] = &[
    "self.browse",
    "self.copy",
    "self.env",
    "self.filtered",
    "self.filtered_domain",
    "self.mapped",
    "self.search",
    "self.sorted",
];

/// Traces `expr` back to a dotted "root" describing what produced the record it's called on,
/// resolving through local assignments and for-loop targets within `function_body` (e.g.
/// `record = self.browse(...)` and `for record in self.search(...):` both resolve to
/// `"self.browse"` / `"self.search"`). `depth` bounds the number of names traced through, to
/// avoid looping on self-referential assignments like `records = records.filtered(...)`.
fn resolve_receiver_root(function_body: &[Stmt], expr: &Expr, depth: u8) -> Option<String> {
    if depth == 0 {
        return None;
    }
    match expr {
        Expr::Name(name) if name.id == "self" => Some("self".to_string()),
        Expr::Name(name) => {
            let mut finder = NameBindingFinder {
                name: name.id.as_str(),
                value: None,
            };
            for stmt in function_body {
                finder.visit_stmt(stmt);
                if finder.value.is_some() {
                    break;
                }
            }
            resolve_receiver_root(function_body, finder.value?, depth - 1)
        }
        Expr::Call(call) => resolve_receiver_root(function_body, &call.func, depth - 1),
        Expr::Subscript(subscript) => {
            resolve_receiver_root(function_body, &subscript.value, depth - 1)
        }
        // `self.env["res.users"].browse(...)`: `dotted_name` can't resolve a chain with a
        // `Subscript` link (`self.env[...]`), so fall back to recursing into the attribute's
        // own receiver, which re-enters the `Subscript` arm above and resolves to `self.env`.
        Expr::Attribute(attribute) => dotted_name(expr)
            .or_else(|| resolve_receiver_root(function_body, &attribute.value, depth - 1)),
        _ => None,
    }
}

/// Finds the first assignment (`name = value`) or for-loop target (`for name in value:`) for
/// `name` anywhere in a method body, including nested blocks (`if`, `for`, `try`, ...).
struct NameBindingFinder<'a> {
    name: &'a str,
    value: Option<&'a Expr>,
}

impl<'a> Visitor<'a> for NameBindingFinder<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if self.value.is_some() {
            return;
        }
        match stmt {
            Stmt::Assign(assign)
                if assign
                    .targets
                    .iter()
                    .any(|target| matches!(target, Expr::Name(n) if n.id == self.name)) =>
            {
                self.value = Some(&assign.value);
            }
            Stmt::For(for_stmt) if matches!(for_stmt.target.as_ref(), Expr::Name(n) if n.id == self.name) =>
            {
                self.value = Some(&for_stmt.iter);
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

/// Collects the ranges of `.write(...)` calls, on `self` or on a recordset derived from it
/// (see [`RECORDSET_ROOT_METHODS`]), in a method body.
struct WriteCallCollector<'a> {
    function_body: &'a [Stmt],
    ranges: Vec<TextRange>,
}

impl<'a> Visitor<'a> for WriteCallCollector<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref()
            && attr == "write"
            && let Some(root) = resolve_receiver_root(self.function_body, value, 8)
            && (root == "self"
                || RECORDSET_ROOT_METHODS.contains(&root.as_str())
                || root.starts_with("self.with_"))
        {
            self.ranges.push(call.range());
        }
        walk_expr(self, expr);
    }
}

/// ODE8135
pub(crate) fn no_write_in_compute(checker: &Checker, class_def: &ast::StmtClassDef) {
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }

    // Collect the method names referenced by `compute=` field arguments in this class.
    let mut compute_methods: Vec<&str> = Vec::new();
    for stmt in &class_def.body {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        let Expr::Call(call) = assign.value.as_ref() else {
            continue;
        };
        if odoo_field_type(&call.func).is_none() {
            continue;
        }
        for keyword in &call.arguments.keywords {
            if keyword.arg.as_deref() != Some("compute") {
                continue;
            }
            match &keyword.value {
                Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                    compute_methods.push(value.to_str());
                }
                Expr::Name(ast::ExprName { id, .. }) => compute_methods.push(id.as_str()),
                _ => {}
            }
        }
    }
    if compute_methods.is_empty() {
        return;
    }

    for stmt in &class_def.body {
        let Stmt::FunctionDef(function_def) = stmt else {
            continue;
        };
        if !compute_methods.contains(&function_def.name.as_str()) {
            continue;
        }
        let mut collector = WriteCallCollector {
            function_body: &function_def.body,
            ranges: Vec::new(),
        };
        for stmt in &function_def.body {
            collector.visit_stmt(stmt);
        }
        for range in collector.ranges {
            checker.report_diagnostic(NoWriteInCompute, range);
        }
    }
}
