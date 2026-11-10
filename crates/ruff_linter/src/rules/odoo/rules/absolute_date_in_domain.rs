use std::fmt::Write;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_python_ast::{self as ast, Expr, Keyword, Number, Operator};
use ruff_python_parser::parse_expression;
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for a domain condition that compares a field with the current date written as a
/// Python expression, instead of Odoo's relative date syntax (`'today'`, `'today +1d'`,
/// `'now'`, `'today -1m'`).
///
/// It reads the domains of an XML file (the `domain` and `filter_domain` attributes,
/// `<attribute name="domain">`, and the `domain` and `domain_force` fields of a record) and the
/// `domain=` argument of a Python field written as a string, which Odoo evaluates in the
/// browser. A condition is reported when its value is built from:
///
/// - `time.strftime(...)`;
/// - `context_today()` or `current_date`;
/// - `datetime.date.today()`, `datetime.datetime.now()` or `datetime.datetime.today()`;
///
/// alone, with `.strftime(...)`, or moved by `+`/`-` a `relativedelta(...)` or `timedelta(...)`.
/// A domain in Python code is not read: `fields.Date.context_today(self)` there is the user's
/// day and is correct.
///
/// ## Why is this bad?
/// The expression is evaluated when the domain is, and in the web client `time.strftime`,
/// `context_today()` and `datetime.datetime.now()` read the clock in UTC, so for a user whose
/// timezone is not UTC the "day" is the UTC one: a filter "created today" is hours off, and
/// the day changes at the wrong time. The relative syntax is resolved by the server with
/// the timezone of the user.
///
/// It is also easy to write wrongly. A condition `=` against a date on a `Datetime` field
/// matches only the instant of midnight, and a range `>= 'today'`, `<= 'today'` is the instant
/// of midnight too, since `<=` is widened to the end of the day only for a date literal, never
/// for a relative one. The end of the day is `< 'today +1d'`. Measured on an Odoo 19.0 registry,
/// for the user's day `D`, the conditions that mean the same on a `Date` and on a `Datetime` are:
///
/// | Old value | `>=` | `<` | `<=` | `>` | `=` |
/// | --- | --- | --- | --- | --- | --- |
/// | `context_today()` or `strftime('%Y-%m-%d')` | `>= 'today'` | `< 'today'` | `< 'today +1d'` | `>= 'today +1d'` | `>= 'today'` and `< 'today +1d'` |
/// | `strftime('%Y-%m-%d 00:00:00')` | `>= 'today'` | `< 'today'` | `<= 'today'` | `> 'today'` | `= 'today'` |
/// | `strftime('%Y-%m-%d 23:59:59')` | none | `< 'today +1d'` | `< 'today +1d'` | none | none |
///
/// An offset moves the argument (`context_today() - relativedelta(days=3)` is `'today -3d'`),
/// and the `+1d` of the end of the day is added to it.
///
/// The message names the replacement when the expression is one of the above. For anything
/// else (`'%Y-%m-01'`, a `relativedelta` with an argument that is not a number) it only asks
/// for the relative syntax. This rule applies from Odoo 19.0 on, where the relative syntax
/// exists, and it does not offer a fix: the XML files are never rewritten by `check --fix`, and
/// the condition may become two.
///
/// ## Example
/// ```xml
/// <filter
///     name="today"
///     string="Created Today"
///     domain="[('create_date', '>=', time.strftime('%Y-%m-%d 00:00:00')),
///              ('create_date', '<=', time.strftime('%Y-%m-%d 23:59:59'))]"
/// />
/// ```
///
/// Use instead:
/// ```xml
/// <filter
///     name="today"
///     string="Created Today"
///     domain="[('create_date', '>=', 'today'), ('create_date', '<', 'today +1d')]"
/// />
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.17.0.36", category = Category::Suspicious)]
pub(crate) struct AbsoluteDateInDomain {
    condition: String,
    replacement: Option<String>,
}

impl Violation for AbsoluteDateInDomain {
    #[derive_message_formats]
    fn message(&self) -> String {
        let AbsoluteDateInDomain {
            condition,
            replacement,
        } = self;
        match replacement {
            Some(replacement) => format!(
                "The domain condition `{condition}` uses the date of the evaluation; use the relative date `{replacement}`"
            ),
            None => format!(
                "The domain condition `{condition}` uses the date of the evaluation; use the relative date syntax (`'today'`, `'today +1d'`, `'now'`)"
            ),
        }
    }
}

/// What the value of the condition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A day: a date, or a date formatted as `%Y-%m-%d`.
    Day,
    /// The start of a day: `%Y-%m-%d 00:00:00`.
    Start,
    /// The end of a day: `%Y-%m-%d 23:59:59`.
    End,
    /// The current instant.
    Instant,
    /// A date formatted in another way (`%Y-%m-01`): reported without a replacement.
    Other,
}

/// The shift of a date, in the units the relative syntax has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Offset {
    years: i64,
    months: i64,
    weeks: i64,
    days: i64,
}

impl Offset {
    fn scaled(self, sign: i64) -> Offset {
        Offset {
            years: self.years * sign,
            months: self.months * sign,
            weeks: self.weeks * sign,
            days: self.days * sign,
        }
    }

    fn plus(self, other: Offset) -> Offset {
        Offset {
            years: self.years + other.years,
            months: self.months + other.months,
            weeks: self.weeks + other.weeks,
            days: self.days + other.days,
        }
    }

    /// `today +1d`, `now`, `today -1m +1d`: the relative date that starts at `origin`.
    fn relative(self, origin: &str, extra_days: i64) -> String {
        let mut text = origin.to_string();
        for (amount, unit) in [
            (self.years, 'y'),
            (self.months, 'm'),
            (self.weeks, 'w'),
            (self.days + extra_days, 'd'),
        ] {
            if amount != 0 {
                let _ = write!(text, " {amount:+}{unit}");
            }
        }
        text
    }
}

/// A value read from a condition: what it is and how far it is moved. `offset` is `None` when
/// the shift is not made of constant days, weeks, months or years.
#[derive(Debug, Clone, Copy)]
struct DateValue {
    kind: Kind,
    offset: Option<Offset>,
}

/// Where the condition being looked at is written.
struct Condition<'a> {
    /// The text that the ranges of the condition index into: the whole file, or the contents of
    /// a string literal that holds a domain.
    text: &'a str,
    /// Where `text` begins in the file.
    shift: TextSize,
}

/// ODW9506, for a domain written in an XML file, which is linted as an expression with
/// `domain_field` set. A domain in Python code is never read.
pub(crate) fn absolute_date_in_domain(checker: &Checker, elts: &[Expr]) {
    if !checker.settings().odoo.domain_field {
        return;
    }
    let condition = Condition {
        text: checker.source(),
        shift: TextSize::default(),
    };
    check_condition(checker, elts, &condition);
}

/// ODW9506, for the domain that a `domain=` argument holds as a string, such as the one of a
/// field: `fields.Many2one(domain="[('date', '>=', context_today())]")`.
///
/// A string that is concatenated, an f-string, a byte string or one with an escape is left
/// alone, since its contents are not the text of the file.
pub(crate) fn absolute_date_in_domain_string(checker: &Checker, keywords: &[Keyword]) {
    for keyword in keywords {
        if keyword.arg.as_deref() != Some("domain") {
            continue;
        }
        let Expr::StringLiteral(string) = &keyword.value else {
            continue;
        };
        let Some(literal) = string.as_single_part_string() else {
            continue;
        };
        let contents = checker.locator().slice(literal.content_range());
        if contents != &*literal.value {
            continue;
        }
        let trimmed = contents.trim_start();
        let Ok(parsed) = parse_expression(trimmed) else {
            continue;
        };
        let leading = TextSize::try_from(contents.len() - trimmed.len()).unwrap_or_default();
        let condition = Condition {
            text: trimmed,
            shift: literal.content_range().start() + leading,
        };
        let mut finder = ConditionFinder {
            checker,
            condition: &condition,
        };
        finder.visit_expr(parsed.expr());
    }
}

/// Looks at every three-item list or tuple of a parsed domain.
struct ConditionFinder<'a, 'b> {
    checker: &'a Checker<'b>,
    condition: &'a Condition<'a>,
}

impl<'a> Visitor<'a> for ConditionFinder<'_, '_> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) =
            expr
        {
            check_condition(self.checker, elts, self.condition);
        }
        walk_expr(self, expr);
    }
}

fn check_condition(checker: &Checker, elts: &[Expr], condition: &Condition) {
    // The relative date syntax is Odoo 19.0.
    if !odoo_version_applies(checker, Some(OdooVersion::new(19, 0)), None) {
        return;
    }
    let [field, operator, value] = elts else {
        return;
    };
    let (Some(field_name), Some(operator_name)) = (string_value(field), string_value(operator))
    else {
        return;
    };
    if field_name.is_empty() || !is_comparison(operator_name) {
        return;
    }
    let Some(date) = date_value(value) else {
        return;
    };

    let text = condition_text(condition.text, elts[0].start(), value.end());
    let replacement = replacement(
        slice(condition.text, field.range()),
        slice(condition.text, operator.range()),
        operator_name,
        date,
    );
    checker.report_diagnostic(
        AbsoluteDateInDomain {
            condition: format!("({text})"),
            replacement,
        },
        value.range() + condition.shift,
    );
}

/// The text of the condition from the field to the end of the value. The range of a value does
/// not include the parentheses around it, so the ones left open are closed.
fn condition_text(text: &str, start: TextSize, end: TextSize) -> String {
    let condition = slice(text, TextRange::new(start, end));
    let unclosed = condition
        .matches('(')
        .count()
        .saturating_sub(condition.matches(')').count());
    format!("{condition}{}", ")".repeat(unclosed))
}

fn slice(text: &str, range: TextRange) -> &str {
    &text[range]
}

fn string_value(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::StringLiteral(string) => Some(string.value.to_str()),
        _ => None,
    }
}

/// The operators that bound a field by a date.
fn is_comparison(operator: &str) -> bool {
    matches!(operator, "=" | "!=" | "<>" | "<" | "<=" | ">" | ">=")
}

/// The conditions that replace `(field, operator, value)`, as the text of a domain.
fn replacement(field: &str, operator: &str, name: &str, date: DateValue) -> Option<String> {
    let offset = date.offset?;
    let quote = operator
        .chars()
        .next()
        .filter(|ch| matches!(ch, '"' | '\''))?;
    let condition = |name: &str, relative: String| {
        format!("({field}, {quote}{name}{quote}, {quote}{relative}{quote})")
    };
    let today = |extra_days: i64| offset.relative("today", extra_days);
    let now = || offset.relative("now", 0);
    let conditions = match (date.kind, name) {
        (Kind::Day, ">=") => vec![condition(">=", today(0))],
        (Kind::Day, "<") => vec![condition("<", today(0))],
        (Kind::Day, "<=") => vec![condition("<", today(1))],
        (Kind::Day, ">") => vec![condition(">=", today(1))],
        (Kind::Day, "=") => vec![condition(">=", today(0)), condition("<", today(1))],
        (Kind::Start, "=" | "<" | "<=" | ">" | ">=") => vec![condition(name, today(0))],
        (Kind::End, "<" | "<=") => vec![condition("<", today(1))],
        (Kind::Instant, "=" | "<" | "<=" | ">" | ">=") => vec![condition(name, now())],
        _ => return None,
    };
    Some(conditions.join(", "))
}

/// Reads `value` as a date taken from the clock, or `None` when it is not one.
fn date_value(value: &Expr) -> Option<DateValue> {
    if let Expr::Call(call) = value
        && let Expr::Attribute(attribute) = call.func.as_ref()
        && attribute.attr.as_str() == "strftime"
    {
        let [Expr::StringLiteral(format)] = &*call.arguments.args else {
            return None;
        };
        if !call.arguments.keywords.is_empty() {
            return None;
        }
        // `time.strftime(format)` reads the clock itself.
        let base = if matches!(attribute.value.as_ref(), Expr::Name(name) if name.id.as_str() == "time")
        {
            Base {
                instant: true,
                offset: Some(Offset::default()),
            }
        } else {
            base(&attribute.value)?
        };
        return Some(DateValue {
            kind: kind_of_format(format.value.to_str()),
            offset: base.offset,
        });
    }
    let base = base(value)?;
    Some(DateValue {
        kind: if base.instant {
            Kind::Instant
        } else {
            Kind::Day
        },
        offset: base.offset,
    })
}

fn kind_of_format(format: &str) -> Kind {
    // An Odoo data file requires `%` to be doubled.
    match format.replace("%%", "%").as_str() {
        "%Y-%m-%d" => Kind::Day,
        "%Y-%m-%d 00:00:00" => Kind::Start,
        "%Y-%m-%d 23:59:59" => Kind::End,
        "%Y-%m-%d %H:%M:%S" => Kind::Instant,
        _ => Kind::Other,
    }
}

/// A date taken from the clock, and how far it is moved.
#[derive(Debug, Clone, Copy)]
struct Base {
    /// `datetime.datetime.now()`, as against a date.
    instant: bool,
    offset: Option<Offset>,
}

/// `context_today()`, `datetime.date.today()`, `datetime.datetime.now()`, optionally moved by
/// `+` or `-` a `relativedelta(...)` or `timedelta(...)`.
fn base(expr: &Expr) -> Option<Base> {
    match expr {
        Expr::Name(name) if name.id.as_str() == "current_date" => Some(Base {
            instant: false,
            offset: Some(Offset::default()),
        }),
        Expr::Call(call) => {
            if !call.arguments.is_empty() {
                return None;
            }
            let instant = match dotted(&call.func)?.as_str() {
                "context_today" | "date.today" | "datetime.date.today" => false,
                "datetime.now"
                | "datetime.today"
                | "datetime.datetime.now"
                | "datetime.datetime.today" => true,
                _ => return None,
            };
            Some(Base {
                instant,
                offset: Some(Offset::default()),
            })
        }
        Expr::BinOp(ast::ExprBinOp {
            left, op, right, ..
        }) => {
            let sign = match op {
                Operator::Add => 1,
                Operator::Sub => -1,
                _ => return None,
            };
            let base = base(left)?;
            let delta = delta(right)?;
            Some(Base {
                instant: base.instant,
                offset: match (base.offset, delta) {
                    (Some(offset), Delta::Known(delta)) => Some(offset.plus(delta.scaled(sign))),
                    _ => None,
                },
            })
        }
        _ => None,
    }
}

/// What an operand of `+` or `-` is.
enum Delta {
    /// A `relativedelta(...)` or `timedelta(...)` of constant days, weeks, months or years.
    Known(Offset),
    /// A `relativedelta(...)` or `timedelta(...)` that is read no further.
    Unknown,
}

/// Reads `expr` as a `relativedelta(...)` or `timedelta(...)`, or `None` when it is not one.
fn delta(expr: &Expr) -> Option<Delta> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let is_relativedelta = match dotted(&call.func)?.as_str() {
        "relativedelta" => true,
        "timedelta" | "datetime.timedelta" => false,
        _ => return None,
    };
    if !call.arguments.args.is_empty() {
        return Some(Delta::Unknown);
    }
    let mut offset = Offset::default();
    for keyword in &call.arguments.keywords {
        let Some(amount) = keyword.arg.as_ref().and(integer(&keyword.value)) else {
            return Some(Delta::Unknown);
        };
        match (keyword.arg.as_deref(), is_relativedelta) {
            (Some("days"), _) => offset.days += amount,
            (Some("weeks"), _) => offset.weeks += amount,
            (Some("months"), true) => offset.months += amount,
            (Some("years"), true) => offset.years += amount,
            _ => return Some(Delta::Unknown),
        }
    }
    Some(Delta::Known(offset))
}

fn integer(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::NumberLiteral(ast::ExprNumberLiteral {
            value: Number::Int(int),
            ..
        }) => int.as_i64(),
        Expr::UnaryOp(ast::ExprUnaryOp {
            op: ast::UnaryOp::USub,
            operand,
            ..
        }) => integer(operand).map(|amount| -amount),
        Expr::UnaryOp(ast::ExprUnaryOp {
            op: ast::UnaryOp::UAdd,
            operand,
            ..
        }) => integer(operand),
        _ => None,
    }
}

/// `a.b.c` as text, for a chain of attributes that starts at a name.
fn dotted(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => {
            Some(format!("{}.{}", dotted(&attribute.value)?, attribute.attr))
        }
        _ => None,
    }
}
