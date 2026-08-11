use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_field_type};

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
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct NoWriteInCompute;

impl Violation for NoWriteInCompute {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Compute method calling `write`. Use `update` instead.".to_string()
    }
}

/// Collects the ranges of `self.write(...)` calls in a method body.
struct WriteCallCollector {
    ranges: Vec<TextRange>,
}

impl<'a> Visitor<'a> for WriteCallCollector {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref()
            && attr == "write"
            && matches!(value.as_ref(), Expr::Name(name) if name.id == "self")
        {
            self.ranges.push(call.range());
        }
        walk_expr(self, expr);
    }
}

/// ODOO040
pub(crate) fn no_write_in_compute(checker: &Checker, class_def: &ast::StmtClassDef) {
    if !is_odoo_model_class(class_def) {
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
        let mut collector = WriteCallCollector { ranges: Vec::new() };
        for stmt in &function_def.body {
            collector.visit_stmt(stmt);
        }
        for range in collector.ranges {
            checker.report_diagnostic(NoWriteInCompute, range);
        }
    }
}
