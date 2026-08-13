use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_python_semantic::analyze::typing::find_assigned_value;
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::is_odoo_model_class;

/// ## What it does
/// Checks for `search([])`/`search_read([])` calls with an empty domain and no `limit`.
///
/// ## Why is this bad?
/// An empty domain without a limit loads *all* records of the model, which can be a serious
/// performance problem on large tables.
///
/// ## Example
/// ```python
/// partners = self.env["res.partner"].search([])
/// ```
///
/// Use instead:
/// ```python
/// partners = self.env["res.partner"].search([], limit=100)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct NoSearchAll {
    method: String,
}

impl Violation for NoSearchAll {
    #[derive_message_formats]
    fn message(&self) -> String {
        let NoSearchAll { method } = self;
        format!(
            "Using an empty domain `{method}([])` without a `limit` will load all records, may impact performance."
        )
    }
}

/// ODOO038
pub(crate) fn no_search_all(checker: &Checker, call: &ast::ExprCall) {
    let method = match call.func.as_ref() {
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str(),
        Expr::Name(ast::ExprName { id, .. }) => id.as_str(),
        _ => return,
    };
    if !matches!(method, "search" | "search_read") {
        return;
    }
    if call.arguments.is_empty() {
        return;
    }

    // Only inside a method of an Odoo model class, mirroring pylint-odoo's scoping.
    let ScopeKind::Function(function_def) = checker.semantic().current_scope().kind else {
        return;
    };
    let semantic = checker.semantic();
    if !semantic.current_scopes().any(
        |scope| matches!(scope.kind, ScopeKind::Class(class_def) if is_odoo_model_class(semantic, class_def)),
    ) {
        return;
    }

    let domain = call.arguments.args.first().or_else(|| {
        call.arguments
            .keywords
            .iter()
            .find(|keyword| keyword.arg.as_deref() == Some("domain"))
            .map(|keyword| &keyword.value)
    });
    let Some(domain) = domain else {
        return;
    };
    if !is_empty_domain(checker, function_def, domain, call.start()) {
        return;
    }

    let has_limit_or_count = call
        .arguments
        .keywords
        .iter()
        .any(|keyword| matches!(keyword.arg.as_deref(), Some("limit" | "count")))
        || call.arguments.args.len() >= 3
        || (method == "search" && call.arguments.args.len() >= 5);
    if has_limit_or_count {
        return;
    }

    checker.report_diagnostic(
        NoSearchAll {
            method: method.to_string(),
        },
        call.range(),
    );
}

/// Returns `true` if `domain` is an empty-list literal, or a `Name` assigned (within
/// `function_def`, the enclosing method) an empty-list literal with no
/// `.append`/`.extend`/`.insert` call on it between the assignment and `call_start` —
/// mirroring pylint-odoo's handling of `domain = []; search(domain)`.
fn is_empty_domain(
    checker: &Checker,
    function_def: &ast::StmtFunctionDef,
    domain: &Expr,
    call_start: TextSize,
) -> bool {
    match domain {
        Expr::List(ast::ExprList { elts, .. }) => elts.is_empty(),
        Expr::Name(name) => {
            let Some(Expr::List(list)) = find_assigned_value(name.id.as_str(), checker.semantic())
            else {
                return false;
            };
            if !list.elts.is_empty() {
                return false;
            }
            let mut collector = DomainMutationVisitor {
                name: name.id.as_str(),
                range: TextRange::new(list.range().end(), call_start),
                found: false,
            };
            for stmt in &function_def.body {
                collector.visit_stmt(stmt);
            }
            !collector.found
        }
        _ => false,
    }
}

/// Detects a `<name>.append(...)`/`.extend(...)`/`.insert(...)` call within `range`.
struct DomainMutationVisitor<'a> {
    name: &'a str,
    range: TextRange,
    found: bool,
}

impl<'a> Visitor<'a> for DomainMutationVisitor<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if !self.found
            && let Expr::Call(call) = expr
            && self.range.contains(call.start())
            && let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref()
            && matches!(attr.as_str(), "append" | "extend" | "insert")
            && matches!(value.as_ref(), Expr::Name(name) if name.id == self.name)
        {
            self.found = true;
        }
        walk_expr(self, expr);
    }
}
