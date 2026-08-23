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
/// Checks for `search([])`/`search_read([])` calls with an empty domain and no `limit` on a
/// model known to hold a large number of records.
///
/// ## Why is this bad?
/// An empty domain without a limit loads *all* records of the model. On the tables that grow
/// without bound in a running Odoo database — journal entries, stock moves, messages,
/// attachments — that is a serious performance problem.
///
/// The model the call runs against is resolved from `self.env["..."]` (directly or through a
/// local variable) and from the `_name`/`_inherit` of the enclosing model class. A call whose
/// model cannot be resolved — `self.env[model_name]`, the comodel of a relational field — is
/// not reported.
///
/// ## Example
/// ```python
/// moves = self.env["account.move"].search([])
/// ```
///
/// Use instead:
/// ```python
/// moves = self.env["account.move"].search([], limit=100)
/// ```
///
/// ## Options
/// - `lint.odoo.no-search-all-models`
///
/// The default is the models that grow without bound in a running Odoo database. Entries are
/// matched as globs, so `account.move*` covers `account.move` and `account.move.line`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct NoSearchAll {
    method: String,
    model: String,
}

impl Violation for NoSearchAll {
    #[derive_message_formats]
    fn message(&self) -> String {
        let NoSearchAll { method, model } = self;
        format!(
            "Using an empty domain `{method}([])` without a `limit` will load all records of \"{model}\", may impact performance."
        )
    }
}

/// The models whose tables grow without bound in a running Odoo database.
///
/// Entries are globs: `account.move*` covers both the journal entries and their lines.
const HEAVY_MODELS: &[&str] = &[
    "account.analytic.line",
    "account.bank.statement.line",
    "account.move*",
    "account.partial.reconcile",
    "account.payment",
    "bus.bus",
    "crm.lead",
    "hr.attendance",
    "hr.leave",
    "hr.payslip*",
    "ir.attachment",
    "ir.logging",
    "ir.model.data",
    "mail.followers",
    "mail.mail",
    "mail.message",
    "mail.notification",
    "mail.tracking.value",
    "mrp.production",
    "mrp.workorder",
    "payment.transaction",
    "pos.order*",
    "product.product",
    "product.template",
    "project.task",
    "purchase.order*",
    "queue.job",
    "res.partner",
    "sale.order*",
    "sms.sms",
    "stock.move*",
    "stock.picking",
    "stock.quant",
    "stock.valuation.layer",
    "website.track",
    "website.visitor",
];

/// The methods that hand back the same recordset they are called on, so the model survives
/// them.
const RECORDSET_PASSTHROUGH_METHODS: &[&str] = &[
    "exists",
    "sudo",
    "with_company",
    "with_context",
    "with_env",
    "with_user",
];

/// ODW8163
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
    let Some(model_class) = enclosing_model_class(checker) else {
        return;
    };

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

    // A call whose model cannot be resolved is left alone rather than reported blindly.
    let Some(model) = called_model(checker, call, model_class) else {
        return;
    };
    if !checker
        .settings()
        .odoo
        .no_search_all_models
        .matches_glob(&model, HEAVY_MODELS)
    {
        return;
    }

    checker.report_diagnostic(
        NoSearchAll {
            method: method.to_string(),
            model,
        },
        call.range(),
    );
}

/// The Odoo model class the call sits in, walking out through the enclosing scopes.
fn enclosing_model_class<'a>(checker: &'a Checker) -> Option<&'a ast::StmtClassDef> {
    let semantic = checker.semantic();
    semantic
        .current_scopes()
        .find_map(|scope| match scope.kind {
            ScopeKind::Class(class_def) if is_odoo_model_class(semantic, class_def) => {
                Some(class_def)
            }
            _ => None,
        })
}

/// The model `call` runs `search` against, if it can be resolved within this file.
///
/// Three shapes resolve: `self.env["sale.order"].search(...)`, a local bound to that subscript
/// (`sale = self.env["sale.order"]`), and `self.search(...)`, which runs against the model the
/// enclosing class declares.
fn called_model(
    checker: &Checker,
    call: &ast::ExprCall,
    model_class: &ast::StmtClassDef,
) -> Option<String> {
    let Expr::Attribute(ast::ExprAttribute { value, .. }) = call.func.as_ref() else {
        return None;
    };
    match strip_passthrough_calls(value) {
        Expr::Subscript(subscript) => env_subscript_model(subscript),
        Expr::Name(name) => {
            if name.id.as_str() == "self" {
                return declared_model(model_class);
            }
            let assigned = find_assigned_value(name.id.as_str(), checker.semantic())?;
            match strip_passthrough_calls(assigned) {
                Expr::Subscript(subscript) => env_subscript_model(subscript),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Peels off the `.sudo()`/`.with_context(...)`/... calls that hand back the same recordset,
/// so `self.env["account.move"].sudo().search([])` still resolves to `account.move`.
fn strip_passthrough_calls(expr: &Expr) -> &Expr {
    let mut current = expr;
    loop {
        let Expr::Call(call) = current else {
            return current;
        };
        let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
            return current;
        };
        if !RECORDSET_PASSTHROUGH_METHODS.contains(&attr.as_str()) {
            return current;
        }
        current = value;
    }
}

/// The model named by an `<anything>.env["model.name"]` subscript.
fn env_subscript_model(subscript: &ast::ExprSubscript) -> Option<String> {
    let Expr::Attribute(ast::ExprAttribute { attr, .. }) = subscript.value.as_ref() else {
        return None;
    };
    if attr.as_str() != "env" {
        return None;
    }
    let Expr::StringLiteral(literal) = subscript.slice.as_ref() else {
        return None;
    };
    Some(literal.value.to_str().to_string())
}

/// The model `class_def` declares, from its `_name` or, failing that, its `_inherit`.
///
/// A multi-model `_inherit` list names no single model, so it resolves to nothing rather than
/// to an arbitrary entry of the list.
fn declared_model(class_def: &ast::StmtClassDef) -> Option<String> {
    class_attribute_model(class_def, "_name")
        .or_else(|| class_attribute_model(class_def, "_inherit"))
}

/// The string a class-level `<attribute> = "..."` (or single-element list) assignment names.
fn class_attribute_model(class_def: &ast::StmtClassDef, attribute: &str) -> Option<String> {
    class_def.body.iter().find_map(|stmt| {
        let ast::Stmt::Assign(assign) = stmt else {
            return None;
        };
        if !assign
            .targets
            .iter()
            .any(|target| matches!(target, Expr::Name(name) if name.id == attribute))
        {
            return None;
        }
        match assign.value.as_ref() {
            Expr::StringLiteral(literal) => Some(literal.value.to_str().to_string()),
            Expr::List(ast::ExprList { elts, .. }) => match elts.as_slice() {
                [Expr::StringLiteral(literal)] => Some(literal.value.to_str().to_string()),
                _ => None,
            },
            _ => None,
        }
    })
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
