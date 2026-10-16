use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::map_callable;
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{compute_method_names, dotted_name, is_odoo_model_class};
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for the `id` of the record being computed, or changed by an onchange, used as the
/// value of a domain condition, as in `("partner_id", "=", record.id)`.
///
/// The rule looks at the methods a field names in `compute=`, and at the methods decorated
/// with `@api.depends`, `@api.depends_context` or `@api.onchange`. Inside them it reports
/// `self.id`, and `record.id` where `record` is the variable of a loop over `self` (or over
/// `self.filtered(...)`, `self.sorted(...)`, `self.sudo()` and the other methods that keep the
/// same records), when it is the value of a condition: the third item of a `(field, operator,
/// value)` tuple or list, or the third argument of `Domain(field, operator, value)`.
///
/// A loop over `self.filtered("id")`, and a record whose `id` the method tests anywhere
/// (`if record.id:`, `... if record.id else 0`), are not reported: the method already keeps the
/// new records away from the domain.
///
/// ## Why is this bad?
/// Odoo computes fields and runs onchange methods on new records too, the ones a form holds
/// before it is saved, and the `id` of a new record is a `NewId`, not an integer. That is true
/// even for an existing record being edited in a form: the form works on a new record whose
/// origin is the saved one. A domain cannot compare a column to a `NewId`, so the condition
/// never matches what it means to:
/// - Since 19.0 the domain logs `Domains don't support NewId, use .ids instead` and replaces
///   the condition with `("field", "in", [])` (or `"not in"` for a negative operator), so a
///   count over the record's lines is `0` while the user edits it.
/// - Up to 18.0 the result depends on the operator: `=` matches nothing, `child_of` raises
///   `TypeError: 'NewId' object is not iterable`, and `not in` raises `ValueError`.
///
/// `record.ids` is the list of the actual ids behind the records: the origin's id for a new
/// record that has one, and nothing for a record that has none. It is what the domain needs.
///
/// ## Example
/// ```python
/// class ResPartner(models.Model):
///     _inherit = "res.partner"
///
///     registration_count = fields.Integer(compute="_compute_registration_count")
///
///     def _compute_registration_count(self):
///         for partner in self:
///             partner.registration_count = self.env["event.registration"].search_count(
///                 [("attendee_partner_id", "child_of", partner.id)]
///             )
/// ```
///
/// Use instead:
/// ```python
/// class ResPartner(models.Model):
///     _inherit = "res.partner"
///
///     registration_count = fields.Integer(compute="_compute_registration_count")
///
///     def _compute_registration_count(self):
///         for partner in self:
///             partner.registration_count = self.env["event.registration"].search_count(
///                 [("attendee_partner_id", "child_of", partner.ids)]
///             )
/// ```
///
/// ## Fix safety
/// The fix replaces `.id` with `.ids`, and turns `=` into `in` and `!=` into `not in` so that
/// the operator accepts a list; `in`, `not in`, `child_of` and `parent_of` already do. It is
/// not offered for any other operator.
///
/// The fix is marked as unsafe because it changes what the domain matches in two cases. The
/// first is the one the rule is about: on a new record that has an origin, the condition starts
/// matching the origin's rows. The second is a `self.id` on several records, which raises
/// `ValueError: Expected singleton` while `self.ids` holds all of them.
///
/// `record._origin.id` is not reported. It is the saved record's integer id, or `False` for a
/// new record that has no origin; a domain accepts both without a warning, and a condition such
/// as `("id", "!=", record._origin.id)` is correct for either.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.17.0.36", category = Category::Suspicious)]
pub(crate) struct NewIdInDomain {
    record: String,
    rewrite: Rewrite,
}

/// What the fix of a [`NewIdInDomain`] diagnostic rewrites.
#[derive(Debug, PartialEq, Eq)]
enum Rewrite {
    /// `.id` becomes `.ids`, the operator already accepts a list.
    Ids,
    /// `.id` becomes `.ids`, and the operator becomes the one that accepts a list.
    IdsAndOperator(&'static str),
    /// No fix: the operator has no list form, or is not spelled as one plain string.
    None,
}

impl Violation for NewIdInDomain {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let NewIdInDomain { record, .. } = self;
        format!(
            "`{record}.id` is a `NewId` when the record is new, which a domain cannot match; \
             use `{record}.ids`"
        )
    }

    fn fix_title(&self) -> Option<String> {
        let NewIdInDomain { record, rewrite } = self;
        match rewrite {
            Rewrite::Ids => Some(format!("Replace with `{record}.ids`")),
            Rewrite::IdsAndOperator(operator) => {
                Some(format!("Replace with `\"{operator}\", {record}.ids`"))
            }
            Rewrite::None => None,
        }
    }
}

/// The decorators that make a method run on the records being computed or changed in a form.
const RECORD_DECORATORS: &[&str] = &["depends", "depends_context", "onchange"];

/// The recordset methods that return (a subset of) the same records, so a loop over
/// `self.<method>(...)` still visits the records being computed.
const SAME_RECORDS_METHODS: &[&str] = &[
    "exists",
    "filtered",
    "filtered_domain",
    "sorted",
    "sudo",
    "with_company",
    "with_context",
    "with_env",
    "with_prefetch",
    "with_user",
];

/// The operators the domain accepts with a list of ids, mapped to the one the fix writes.
fn list_operator(operator: &str) -> Option<&'static str> {
    match operator {
        "=" | "in" => Some("in"),
        "!=" | "<>" | "not in" => Some("not in"),
        "child_of" => Some("child_of"),
        "parent_of" => Some("parent_of"),
        _ => None,
    }
}

/// The operators of a domain condition, so that a tuple of three items that is not a condition
/// (`("a", "b", record.id)`) is not mistaken for one.
const CONDITION_OPERATORS: &[&str] = &[
    "=",
    "!=",
    "<>",
    "=?",
    "<",
    ">",
    "<=",
    ">=",
    "in",
    "not in",
    "like",
    "not like",
    "ilike",
    "not ilike",
    "=like",
    "=ilike",
    "not =like",
    "not =ilike",
    "child_of",
    "parent_of",
    "any",
    "not any",
    "any!",
    "not any!",
];

/// Returns `true` if the method is decorated with `@api.depends`, `@api.depends_context` or
/// `@api.onchange`.
fn has_record_decorator(checker: &Checker, function_def: &ast::StmtFunctionDef) -> bool {
    function_def.decorator_list.iter().any(|decorator| {
        let callable = map_callable(&decorator.expression);
        if let Some(qualified_name) = checker.semantic().resolve_qualified_name(callable) {
            return matches!(
                qualified_name.segments(),
                ["odoo", "api", name] if RECORD_DECORATORS.contains(name)
            );
        }
        dotted_name(callable).is_some_and(|name| {
            name.strip_prefix("api.")
                .is_some_and(|name| RECORD_DECORATORS.contains(&name))
        })
    })
}

/// Returns `true` if `expr` is `self`, or a chain of [`SAME_RECORDS_METHODS`] calls on it, other
/// than `filtered("id")`, which drops the new records.
fn is_same_records(expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => name.id == "self",
        Expr::Call(call) => {
            let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
                return false;
            };
            if !SAME_RECORDS_METHODS.contains(&attr.as_str()) {
                return false;
            }
            if attr == "filtered"
                && matches!(
                    call.arguments.args.first(),
                    Some(Expr::StringLiteral(literal)) if literal.value.to_str() == "id"
                )
            {
                return false;
            }
            is_same_records(value)
        }
        _ => false,
    }
}

/// The names whose `id` the method tests, in an `if`, an `elif`, a `while`, a conditional
/// expression or an `assert`: `if record.id:`, `if not record.id: continue`,
/// `count if record.id else 0`.
struct TestedNames<'a> {
    names: Vec<&'a str>,
}

impl<'a> TestedNames<'a> {
    fn collect(&mut self, test: &'a Expr) {
        let mut finder = IdAttributeFinder { names: Vec::new() };
        finder.visit_expr(test);
        self.names.extend(finder.names);
    }
}

impl<'a> Visitor<'a> for TestedNames<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::If(ast::StmtIf {
                test,
                elif_else_clauses,
                ..
            }) => {
                self.collect(test);
                for clause in elif_else_clauses {
                    if let Some(test) = &clause.test {
                        self.collect(test);
                    }
                }
            }
            Stmt::While(ast::StmtWhile { test, .. })
            | Stmt::Assert(ast::StmtAssert { test, .. }) => {
                self.collect(test);
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::If(ast::ExprIf { test, .. }) = expr {
            self.collect(test);
        }
        walk_expr(self, expr);
    }
}

/// Collects the `name` of every `name.id` in an expression.
struct IdAttributeFinder<'a> {
    names: Vec<&'a str>,
}

impl<'a> Visitor<'a> for IdAttributeFinder<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Some(name) = id_receiver(expr) {
            self.names.push(name);
        }
        walk_expr(self, expr);
    }
}

/// The name `expr` takes the `id` of, if it is `name.id`.
fn id_receiver(expr: &Expr) -> Option<&str> {
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = expr else {
        return None;
    };
    if attr != "id" {
        return None;
    }
    let Expr::Name(name) = value.as_ref() else {
        return None;
    };
    Some(name.id.as_str())
}

/// A `(field, operator, value)` domain condition whose value is the `id` of the records being
/// computed.
struct Condition<'a> {
    operator: &'a ast::ExprStringLiteral,
    value: &'a ast::ExprAttribute,
    record: &'a str,
}

/// Collects the domain conditions whose value is the `id` of the records being computed.
///
/// Which names hold those records is tracked per loop: the variable of a `for` or of a
/// comprehension is a record of `self` within that loop only, so a name reused by a later loop
/// over other records (`for partner in self.search([])`) is judged by that loop.
struct ConditionCollector<'a> {
    /// The names bound by the enclosing loops, innermost last, with whether each one iterates
    /// over the records being computed.
    loops: Vec<(&'a str, bool)>,
    conditions: Vec<Condition<'a>>,
}

impl<'a> ConditionCollector<'a> {
    /// Returns `true` if `name` holds the records being computed where it is used.
    fn is_record(&self, name: &str) -> bool {
        self.loops
            .iter()
            .rev()
            .find(|(bound, _)| *bound == name)
            .map_or(name == "self", |(_, is_record)| *is_record)
    }

    fn collect(&mut self, items: &'a [Expr]) {
        let [Expr::StringLiteral(_), Expr::StringLiteral(operator), value] = items else {
            return;
        };
        if !CONDITION_OPERATORS.contains(&operator.value.to_str()) {
            return;
        }
        let Expr::Attribute(attribute) = value else {
            return;
        };
        if let Some(record) = id_receiver(value)
            && self.is_record(record)
        {
            self.conditions.push(Condition {
                operator,
                value: attribute,
                record,
            });
        }
    }

    /// Visits `body` with the names of `target` bound to whether `iter` holds the records
    /// being computed.
    fn visit_loop(&mut self, target: &'a Expr, iter: &Expr, body: impl FnOnce(&mut Self)) {
        let is_record = is_same_records(iter);
        let mut finder = NameFinder { names: Vec::new() };
        finder.visit_expr(target);
        let bound = finder.names.len();
        self.loops
            .extend(finder.names.into_iter().map(|name| (name, is_record)));
        body(self);
        self.loops.truncate(self.loops.len() - bound);
    }
}

impl<'a> Visitor<'a> for ConditionCollector<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::For(for_stmt) = stmt {
            self.visit_expr(&for_stmt.iter);
            self.visit_loop(&for_stmt.target, &for_stmt.iter, |collector| {
                collector.visit_body(&for_stmt.body);
            });
            self.visit_body(&for_stmt.orelse);
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
                self.collect(elts);
            }
            Expr::Call(call)
                if call.arguments.keywords.is_empty()
                    && dotted_name(&call.func)
                        .is_some_and(|name| name == "Domain" || name.ends_with(".Domain")) =>
            {
                self.collect(&call.arguments.args);
            }
            _ => {}
        }
        let generators: &'a [ast::Comprehension] = match expr {
            Expr::ListComp(ast::ExprListComp { generators, .. })
            | Expr::SetComp(ast::ExprSetComp { generators, .. })
            | Expr::Generator(ast::ExprGenerator { generators, .. }) => generators,
            Expr::DictComp(ast::ExprDictComp { generators, .. }) => generators,
            _ => {
                walk_expr(self, expr);
                return;
            }
        };
        // A comprehension binds its variables for the whole expression.
        let bound = self.loops.len();
        for generator in generators {
            self.visit_expr(&generator.iter);
            let is_record = is_same_records(&generator.iter);
            let mut finder = NameFinder { names: Vec::new() };
            finder.visit_expr(&generator.target);
            self.loops
                .extend(finder.names.into_iter().map(|name| (name, is_record)));
        }
        match expr {
            Expr::ListComp(ast::ExprListComp { elt, .. })
            | Expr::SetComp(ast::ExprSetComp { elt, .. })
            | Expr::Generator(ast::ExprGenerator { elt, .. }) => self.visit_expr(elt),
            Expr::DictComp(ast::ExprDictComp { key, value, .. }) => {
                if let Some(key) = key {
                    self.visit_expr(key);
                }
                self.visit_expr(value);
            }
            _ => {}
        }
        for generator in generators {
            for condition in &generator.ifs {
                self.visit_expr(condition);
            }
        }
        self.loops.truncate(bound);
    }
}

/// Collects the names an assignment target binds: `record`, or each of `index, record`.
struct NameFinder<'a> {
    names: Vec<&'a str>,
}

impl<'a> Visitor<'a> for NameFinder<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Name(name) = expr {
            self.names.push(name.id.as_str());
        }
        walk_expr(self, expr);
    }
}

/// ODW9504
pub(crate) fn new_id_in_domain(checker: &Checker, class_def: &ast::StmtClassDef) {
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }
    let compute_methods = compute_method_names(class_def);

    for stmt in &class_def.body {
        let Stmt::FunctionDef(function_def) = stmt else {
            continue;
        };
        if !compute_methods.contains(&function_def.name.as_str())
            && !has_record_decorator(checker, function_def)
        {
            continue;
        }

        let mut tested = TestedNames { names: Vec::new() };
        let mut collector = ConditionCollector {
            loops: Vec::new(),
            conditions: Vec::new(),
        };
        tested.visit_body(&function_def.body);
        collector.visit_body(&function_def.body);

        for condition in collector.conditions {
            if !tested.names.contains(&condition.record) {
                report(checker, &condition);
            }
        }
    }
}

fn report(checker: &Checker, condition: &Condition) {
    let operator = condition.operator.value.to_str();
    let rewrite = match list_operator(operator) {
        Some(new_operator) if new_operator == operator => Rewrite::Ids,
        // Rewrite the operator inside its own quotes, so the literal keeps its quoting. A
        // literal spelled any other way than one plain string (`"!" "="`) is left to the user.
        Some(new_operator) if !condition.operator.value.is_implicit_concatenated() => {
            Rewrite::IdsAndOperator(new_operator)
        }
        _ => Rewrite::None,
    };
    let fix = match rewrite {
        Rewrite::Ids => Some(Fix::unsafe_edit(ids_edit(condition))),
        Rewrite::IdsAndOperator(new_operator) => {
            let source = checker.locator().slice(condition.operator);
            source
                .chars()
                .next()
                .filter(|c| matches!(c, '"' | '\''))
                .map(|quote| {
                    Fix::unsafe_edits(
                        Edit::range_replacement(
                            format!("{quote}{new_operator}{quote}"),
                            condition.operator.range(),
                        ),
                        [ids_edit(condition)],
                    )
                })
        }
        Rewrite::None => None,
    };
    let mut diagnostic = checker.report_diagnostic(
        NewIdInDomain {
            record: condition.record.to_string(),
            rewrite: if fix.is_some() {
                rewrite
            } else {
                Rewrite::None
            },
        },
        condition.value.range(),
    );
    if let Some(fix) = fix {
        diagnostic.set_fix(fix);
    }
}

/// Replaces the `id` of `record.id` with `ids`.
fn ids_edit(condition: &Condition) -> Edit {
    Edit::range_replacement("ids".to_string(), condition.value.attr.range())
}
