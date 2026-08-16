use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::comparable::ComparableExpr;
use ruff_python_ast::statement_visitor::{StatementVisitor, walk_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::dotted_name;

/// ## What it does
/// Checks for cursor `execute`/`executemany` calls whose query is built with string
/// formatting or concatenation from non-constant values, e.g.
/// `cr.execute("... %s" % name)` or `cr.execute(f"... {name}")`.
///
/// ## Why is this bad?
/// Interpolating values into SQL text lets crafted input change the query itself (SQL
/// injection). Values must be passed as query parameters
/// (`cr.execute("... %s", (name,))`) so the database driver escapes them.
///
/// Formatting with constants, `self._table`-style private attributes/methods, or
/// `psycopg2.sql` objects is allowed: those can't be controlled by user input.
///
/// ## Example
/// ```python
/// self.env.cr.execute("SELECT id FROM res_partner WHERE name = '%s'" % name)
/// ```
///
/// Use instead:
/// ```python
/// self.env.cr.execute("SELECT id FROM res_partner WHERE name = %s", (name,))
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.5")]
pub(crate) struct SqlInjection;

impl Violation for SqlInjection {
    #[derive_message_formats]
    fn message(&self) -> String {
        "SQL injection risk. Use parameters if you can. - More info \
         https://github.com/OCA/odoo-community.org/blob/master/website/Contribution/CONTRIBUTING.rst#no-sql-injection"
            .to_string()
    }
}

/// Same default list as pylint-odoo's `cursor-expr` option.
const CURSOR_EXPRS: &[&str] = &["cr", "self._cr", "self.cr", "self.env.cr"];

/// Returns the value assigned to `expr`, to check how a query variable was built
/// (`query = "..." % name; cr.execute(query)`).
///
/// For a plain name, resolves through its binding. Subscript targets (`var[1] = "..." %
/// name; cr.execute(var[1])`) have no binding in Ruff's semantic model, so instead this
/// walks the enclosing function for an `Assign` whose target is structurally identical,
/// matching pylint-odoo's `_get_assignation_nodes`, which compares assignment targets by
/// source text within the same function.
fn assigned_value<'a>(checker: &'a Checker, expr: &'a Expr) -> Option<&'a Expr> {
    match expr {
        Expr::Name(name) => {
            let semantic = checker.semantic();
            let binding = semantic.binding(semantic.resolve_name(name)?);
            match binding.statement(semantic)? {
                Stmt::Assign(assign) => Some(&assign.value),
                Stmt::AnnAssign(assign) => assign.value.as_deref(),
                _ => None,
            }
        }
        Expr::Subscript(_) => assigned_subscript_value(checker, expr),
        _ => None,
    }
}

/// Collects the value most recently assigned to `target` (a subscript expression) while
/// walking a function body.
struct AssignedSubscriptVisitor<'a> {
    target: ComparableExpr<'a>,
    value: Option<&'a Expr>,
}

impl<'a> StatementVisitor<'a> for AssignedSubscriptVisitor<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::Assign(assign) = stmt {
            if assign
                .targets
                .iter()
                .any(|assigned| ComparableExpr::from(assigned) == self.target)
            {
                self.value = Some(&assign.value);
            }
        }
        walk_stmt(self, stmt);
    }
}

/// Returns the value most recently assigned to `target` (a subscript expression) anywhere
/// in the enclosing function.
fn assigned_subscript_value<'a>(checker: &'a Checker, target: &'a Expr) -> Option<&'a Expr> {
    let ScopeKind::Function(function_def) = checker.semantic().current_scope().kind else {
        return None;
    };
    let mut visitor = AssignedSubscriptVisitor {
        target: ComparableExpr::from(target),
        value: None,
    };
    visitor.visit_body(&function_def.body);
    visitor.value
}

/// Returns `true` if `expr` builds a `psycopg2.sql` object (`sql.SQL(...)`,
/// `sql.Identifier(...)`, ...), directly or through a variable.
fn is_psycopg2_sql(checker: &Checker, expr: &Expr) -> bool {
    if let Some(value) = assigned_value(checker, expr) {
        return is_psycopg2_sql(checker, value);
    }
    let Expr::Call(call) = expr else {
        return false;
    };
    checker
        .semantic()
        .resolve_qualified_name(&call.func)
        .is_some_and(|qualified_name| qualified_name.segments().first() == Some(&"psycopg2"))
}

/// Returns `true` for values that can't be injected: constants, `psycopg2.sql` objects,
/// and `self._table`-style private attributes (or calls on them, a common pattern for
/// report helpers like `self._select()`).
fn allowable(checker: &Checker, expr: &Expr) -> bool {
    if is_psycopg2_sql(checker, expr) {
        return true;
    }
    let expr = match expr {
        Expr::Call(call) => call.func.as_ref(),
        _ => expr,
    };
    match expr {
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            matches!(value.as_ref(), Expr::Name(_)) && attr.starts_with('_')
        }
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_) => true,
        _ => false,
    }
}

/// Returns `true` if `expr` builds a query string by interpolating non-allowable values:
/// `%`/`+` operators, `.format(...)`, or f-string interpolations.
fn is_risky_query(checker: &Checker, expr: &Expr) -> bool {
    match expr {
        Expr::BinOp(ast::ExprBinOp {
            op: ast::Operator::Mod | ast::Operator::Add,
            left,
            right,
            ..
        }) => {
            let right_risky = match right.as_ref() {
                // execute("..." % (self._table, name))
                Expr::Tuple(ast::ExprTuple { elts, .. }) => {
                    !elts.iter().all(|elt| allowable(checker, elt))
                }
                // execute("..." % {"table": self._table})
                Expr::Dict(dict) => !dict
                    .items
                    .iter()
                    .all(|item| allowable(checker, &item.value)),
                // execute("..." % name)
                _ => !allowable(checker, right),
            };
            if right_risky {
                return true;
            }
            // execute("SELECT " + operator + " FROM table"): recurse into chained
            // concatenations on the left side.
            !allowable(checker, left) && is_risky_query(checker, left)
        }
        // execute("...".format(self._table, table=name)); psycopg2's sql.SQL(...).format
        // never reaches here because its arguments are `sql.Identifier(...)`-style
        // allowable values.
        Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) if matches!(
            func.as_ref(),
            Expr::Attribute(ast::ExprAttribute { attr, .. }) if attr == "format"
        ) =>
        {
            !arguments.args.iter().all(|arg| allowable(checker, arg))
                || !arguments
                    .keywords
                    .iter()
                    .all(|keyword| allowable(checker, &keyword.value))
        }
        // execute(f"... {name}")
        Expr::FString(fstring) => !fstring
            .value
            .elements()
            .filter_map(ast::InterpolatedStringElement::as_interpolation)
            .all(|interpolation| allowable(checker, &interpolation.expression)),
        _ => false,
    }
}

/// ODE8103
pub(crate) fn sql_injection(checker: &Checker, call: &ast::ExprCall, path: &Path) {
    // Matching pylint-odoo: queries in test files often format constants on purpose.
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("test_"))
    {
        return;
    }
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
        return;
    };
    if !matches!(attr.as_str(), "execute" | "executemany") {
        return;
    }
    let Some(cursor) = dotted_name(value) else {
        return;
    };
    if !CURSOR_EXPRS.contains(&cursor.as_str()) {
        return;
    }
    // A second positional argument means values are (probably) passed as parameters and
    // any formatting on the query has a reason, e.g. splicing `self._table`.
    if call.arguments.args.len() != 1 {
        return;
    }

    let query = &call.arguments.args[0];
    let risky = is_risky_query(checker, query)
        || assigned_value(checker, query).is_some_and(|value| is_risky_query(checker, value));
    if risky {
        checker.report_diagnostic(SqlInjection, call.range());
    }
}
