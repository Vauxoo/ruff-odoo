use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Number, UnaryOp};
use ruff_python_semantic::analyze::typing::find_binding_value;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for a domain condition whose operator is `in` or `not in` and whose value is a
/// single value rather than a list, such as `("country_ids", "in", record.country_id.id)`.
///
/// It looks at every `(field, "in" | "not in", value)` triple, so it covers the domains passed
/// to `search`, `search_count`, `search_read`, `filtered_domain`, `_search` or `domain=`, and
/// the ones kept in a variable, in a Python file and in the `domain` of an XML file: the
/// `domain` and `filter_domain` attributes, `<attribute name="domain">`, the `domain` and
/// `domain_force` fields of a record, and `attrs`. XML files are linted when they are passed
/// to Ruff or matched by `extend-include`.
///
/// The value is reported when it is certainly a scalar:
///
/// - a literal: `"draft"`, `5`, `True`, `False`, `None`;
/// - `uid`, or an attribute named `uid` or `_uid` (`self.env.uid`);
/// - the `id` of a record: `partner.id`, `user.id`, `self.env.company.id`;
/// - a local name whose only assignment in the same scope is one of the above.
///
/// In an XML file, where the names are the fields of the form, a bare name that is `id`,
/// `active_id` or ends in `_id` (`donation_type_id`) is reported too: it holds the id of a
/// many2one, or `False`.
///
/// Everything else is left alone, because it may well be a collection: a list, tuple, set or
/// dict display, a comprehension, a recordset, `record.ids`, `records.mapped(...)`, any call,
/// subscript or conditional expression, and any other name. A Python name is never guessed
/// from its spelling: `company_id = self.env.company` holds a recordset, which a domain
/// accepts.
///
/// ## Why is this bad?
/// `in` and `not in` compare a field with a collection. Since 19.0 a domain that gets a scalar
/// logs `The domain condition [...] should have a list value.` and wraps it in a list, which
/// hides the mistake. A falsy scalar (`False`, `None`, `0`, `""`, or the `id` of an empty
/// recordset) never reaches that step: the condition is replaced by a constant, always false
/// for `in` and always true for `not in`, whatever the records hold.
///
/// The step is [`_optimize_in_set`](https://github.com/odoo/odoo/blob/2504d010c2fdb107961a0d57d564d021b36a08c1/odoo/orm/domains.py#L1323-L1339).
///
/// ## Example
/// ```python
/// orphans = self.env["orphan.group"].search([("country_ids", "in", orphan.country_id.id)])
/// ```
///
/// Use instead:
/// ```python
/// orphans = self.env["orphan.group"].search([("country_ids", "in", orphan.country_id.ids)])
/// ```
///
/// ## Fix safety
/// The fix is offered for Python only, and only when the replacement means the same thing:
/// a truthy literal is wrapped in a list (safe), and `.id` becomes `.ids` or `uid` is wrapped in
/// a list (unsafe, because the object may not be a record, and for a record that is new `.id`
/// is a `NewId` while `.ids` is not). A falsy literal and a name are reported without a fix:
/// `[False]` is not the same condition as `False`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.17.0.36", category = Category::Suspicious)]
pub(crate) struct ScalarInDomain {
    condition: String,
    operator: String,
    value: String,
    fix: Option<Rewrite>,
}

/// How the fix rewrites the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rewrite {
    /// `value` becomes `[value]`.
    Wrap,
    /// `record.id` becomes `record.ids`.
    Ids,
}

impl Violation for ScalarInDomain {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let ScalarInDomain {
            condition,
            operator,
            ..
        } = self;
        format!(
            "The domain condition `{condition}` has a scalar value; `{operator}` expects a list"
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some(match self.fix? {
            Rewrite::Wrap => format!("Wrap `{}` in a list", self.value),
            Rewrite::Ids => "Use `.ids` instead of `.id`".to_string(),
        })
    }
}

/// What kind of scalar the value of a condition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scalar {
    /// A literal that is true in Python.
    TruthyLiteral,
    /// A literal that is false in Python: the condition becomes a constant.
    FalsyLiteral,
    /// `uid` or `self.env.uid`.
    Uid,
    /// `record.id`.
    RecordId,
    /// A name that holds one of the above, assigned elsewhere in the scope.
    Variable,
    /// A bare name the XML form fills with an id.
    FormId,
}

/// The names that stand for a field of the form in the domain of an XML view, apart from the
/// ones ending in `_id`.
const FORM_ID_NAMES: &[&str] = &["id", "active_id"];

/// How deep the chain of assignments of a name is followed.
const MAX_BINDING_DEPTH: u8 = 4;

/// ODW9505
pub(crate) fn scalar_in_domain(checker: &Checker, elts: &[Expr]) {
    // The warning, and the constant a falsy scalar becomes, are Odoo 19.0 behaviour.
    if !odoo_version_applies(checker, Some(OdooVersion::new(19, 0)), None) {
        return;
    }
    let [field, operator, value] = elts else {
        return;
    };
    let (Some(field), Some(operator)) = (string_value(field), string_value(operator)) else {
        return;
    };
    // `("in", "not in", "any")` is a list of operators, not a condition.
    if !matches!(operator, "in" | "not in") || is_operator(field) {
        return;
    }
    let in_form = checker.settings().odoo.domain_field;
    let Some(scalar) = classify(checker, value, in_form, 0) else {
        return;
    };

    let source = checker.locator().slice(value);
    let condition = checker
        .locator()
        .slice(TextRange::new(elts[0].start(), value.end()));
    let fix = match (scalar, value) {
        (Scalar::TruthyLiteral, _) => Some((Rewrite::Wrap, Applicability::Safe)),
        (Scalar::Uid, _) => Some((Rewrite::Wrap, Applicability::Unsafe)),
        (Scalar::RecordId, Expr::Attribute(_)) => Some((Rewrite::Ids, Applicability::Unsafe)),
        _ => None,
    };
    let mut diagnostic = checker.report_diagnostic(
        ScalarInDomain {
            condition: format!("({condition})"),
            operator: operator.to_string(),
            value: source.to_string(),
            fix: fix.map(|(rewrite, _)| rewrite),
        },
        value.range(),
    );
    let Some((rewrite, applicability)) = fix else {
        return;
    };
    let edit = match (rewrite, value) {
        (Rewrite::Wrap, _) => Edit::range_replacement(format!("[{source}]"), value.range()),
        (Rewrite::Ids, Expr::Attribute(attribute)) => {
            Edit::range_replacement("ids".to_string(), attribute.attr.range())
        }
        (Rewrite::Ids, _) => return,
    };
    diagnostic.set_fix(match applicability {
        Applicability::Safe => Fix::safe_edit(edit),
        Applicability::Unsafe => Fix::unsafe_edit(edit),
    });
}

#[derive(Clone, Copy)]
enum Applicability {
    Safe,
    Unsafe,
}

fn string_value(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::StringLiteral(string) => Some(string.value.to_str()),
        _ => None,
    }
}

/// The operators of a domain condition, so that a tuple of operators is not mistaken for one.
fn is_operator(text: &str) -> bool {
    matches!(
        text,
        "=" | "!="
            | "<>"
            | "<"
            | ">"
            | "<="
            | ">="
            | "=?"
            | "=like"
            | "like"
            | "not like"
            | "ilike"
            | "not ilike"
            | "=ilike"
            | "in"
            | "not in"
            | "child_of"
            | "parent_of"
            | "any"
            | "not any"
    )
}

/// Decides whether `value` is certainly a scalar, following the assignments of a name.
fn classify(checker: &Checker, value: &Expr, in_form: bool, depth: u8) -> Option<Scalar> {
    match value {
        Expr::StringLiteral(string) => Some(truthiness(!string.value.to_str().is_empty())),
        Expr::BooleanLiteral(boolean) => Some(truthiness(boolean.value)),
        Expr::NoneLiteral(_) => Some(Scalar::FalsyLiteral),
        Expr::NumberLiteral(number) => Some(truthiness(number_is_truthy(&number.value))),
        Expr::UnaryOp(ast::ExprUnaryOp {
            op: UnaryOp::USub | UnaryOp::UAdd,
            operand,
            ..
        }) => match operand.as_ref() {
            Expr::NumberLiteral(number) => Some(truthiness(number_is_truthy(&number.value))),
            _ => None,
        },
        Expr::Attribute(attribute) => match attribute.attr.as_str() {
            "id" => Some(Scalar::RecordId),
            "uid" | "_uid" => Some(Scalar::Uid),
            _ => None,
        },
        Expr::Name(name) => {
            let name = name.id.as_str();
            if name == "uid" {
                return Some(Scalar::Uid);
            }
            if in_form {
                return (FORM_ID_NAMES.contains(&name) || name.ends_with("_id"))
                    .then_some(Scalar::FormId);
            }
            if depth >= MAX_BINDING_DEPTH {
                return None;
            }
            let semantic = checker.semantic();
            let binding = semantic.binding(semantic.only_binding(value.as_name_expr()?)?);
            let assigned = find_binding_value(binding, semantic)?;
            classify(checker, assigned, false, depth + 1).map(|_| Scalar::Variable)
        }
        _ => None,
    }
}

fn truthiness(truthy: bool) -> Scalar {
    if truthy {
        Scalar::TruthyLiteral
    } else {
        Scalar::FalsyLiteral
    }
}

fn number_is_truthy(number: &Number) -> bool {
    match number {
        Number::Int(int) => int.as_u8() != Some(0),
        Number::Float(float) => *float != 0.0,
        Number::Complex { real, imag } => *real != 0.0 || *imag != 0.0,
    }
}
