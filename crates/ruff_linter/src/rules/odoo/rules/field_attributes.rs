use std::borrow::Cow;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::rules::odoo::helpers::{class_defines_method, odoo_field_type};

/// ## What it does
/// Checks that the method name passed to a field's `compute=` argument starts with
/// `_compute_`.
///
/// ## Why is this bad?
/// The `_compute_<field>` prefix is the Odoo naming convention for compute methods, making
/// their role obvious at the definition and call sites.
///
/// ## Example
/// ```python
/// total = fields.Float(compute="_get_total")
/// ```
///
/// Use instead:
/// ```python
/// total = fields.Float(compute="_compute_total")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct MethodCompute;

impl Violation for MethodCompute {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Name of compute method should start with \"_compute_\"".to_string()
    }
}

/// ## What it does
/// Checks that the method name passed to a field's `search=` argument starts with
/// `_search_`.
///
/// ## Why is this bad?
/// The `_search_<field>` prefix is the Odoo naming convention for search methods.
///
/// ## Example
/// ```python
/// total = fields.Float(search="_find_total")
/// ```
///
/// Use instead:
/// ```python
/// total = fields.Float(search="_search_total")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct MethodSearch;

impl Violation for MethodSearch {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Name of search method should start with \"_search_\"".to_string()
    }
}

/// ## What it does
/// Checks that the method name passed to a field's `inverse=` argument starts with
/// `_inverse_`.
///
/// ## Why is this bad?
/// The `_inverse_<field>` prefix is the Odoo naming convention for inverse methods.
///
/// ## Example
/// ```python
/// total = fields.Float(inverse="_set_total")
/// ```
///
/// Use instead:
/// ```python
/// total = fields.Float(inverse="_inverse_total")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct MethodInverse;

impl Violation for MethodInverse {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Name of inverse method should start with \"_inverse_\"".to_string()
    }
}

/// ## What it does
/// Checks for field arguments that were renamed in newer Odoo versions
/// (`digits_compute` → `digits`, `select` → `index`).
///
/// ## Why is this bad?
/// The old parameter names are silently ignored by the current ORM, so the intended
/// behavior (precision, indexing) is lost.
///
/// ## Example
/// ```python
/// amount = fields.Float(digits_compute=get_precision("Account"))
/// ```
///
/// Use instead:
/// ```python
/// amount = fields.Float(digits=get_precision("Account"))
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct RenamedFieldParameter {
    old: String,
    new: String,
}

impl Violation for RenamedFieldParameter {
    #[derive_message_formats]
    fn message(&self) -> String {
        let RenamedFieldParameter { old, new } = self;
        format!("Field parameter \"{old}\" is no longer supported. Use \"{new}\" instead.")
    }
}

/// ## What it does
/// Checks for `_()` translation calls inside field definitions.
///
/// ## Why is this bad?
/// Field attribute strings (like `string=` and `help=`) are translated automatically by
/// Odoo's export machinery; wrapping them in `_()` is unnecessary and translates them at
/// module-load time, before the user's language is even known.
///
/// ## Example
/// ```python
/// name = fields.Char(string=_("Name"))
/// ```
///
/// Use instead:
/// ```python
/// name = fields.Char(string="Name")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct TranslationField;

impl Violation for TranslationField {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Translation method _(\"string\") in fields is not necessary".to_string()
    }
}

/// ## What it does
/// Checks for `compute=`/`search=`/`inverse=` field arguments that pass a direct method
/// reference instead of the method's name as a string.
///
/// ## Why is this bad?
/// A direct reference binds the field to that exact function object, so a subclass
/// overriding the method is silently ignored. Passing the name as a string preserves
/// inheritability.
///
/// ## Example
/// ```python
/// total = fields.Float(compute=_compute_total)
/// ```
///
/// Use instead:
/// ```python
/// total = fields.Float(compute="_compute_total")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct InheritableMethodString {
    name: String,
}

impl Violation for InheritableMethodString {
    #[derive_message_formats]
    fn message(&self) -> String {
        let InheritableMethodString { name } = self;
        format!("Use string method name `\"{name}\"` to preserve inheritability")
    }
}

/// ## What it does
/// Checks for `default=`/`domain=` field arguments that pass a direct method reference
/// instead of a lambda.
///
/// ## Why is this bad?
/// A direct reference binds the field to that exact function object, so a subclass
/// overriding the method is silently ignored. A lambda dispatches through `self` and
/// preserves inheritability.
///
/// ## Example
/// ```python
/// company_id = fields.Many2one("res.company", default=_default_company)
/// ```
///
/// Use instead:
///
/// ```python
/// company_id = fields.Many2one(
///     "res.company", default=lambda self: self._default_company()
/// )
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct InheritableMethodLambda {
    argument: String,
    name: String,
}

impl Violation for InheritableMethodLambda {
    #[derive_message_formats]
    fn message(&self) -> String {
        let InheritableMethodLambda { argument, name } = self;
        format!("Use `{argument}=lambda self: self.{name}()` to preserve inheritability")
    }
}

/// Old field parameter name → its current replacement.
const RENAMED_PARAMETERS: &[(&str, &str)] = &[("digits_compute", "digits"), ("select", "index")];

/// Returns the string value of `expr`, if it's a plain string literal or an f-string with no
/// interpolations (e.g. `f"_compute_foo"`), mirroring pylint-odoo's `_get_str_value`, which
/// reconstructs an f-string's value from its literal segments.
fn field_attribute_string_value(expr: &Expr) -> Option<Cow<'_, str>> {
    match expr {
        Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
            Some(Cow::Borrowed(value.to_str()))
        }
        Expr::FString(fstring) => {
            let has_interpolation = fstring.value.f_strings().any(|f_string| {
                f_string
                    .elements
                    .iter()
                    .any(ast::InterpolatedStringElement::is_interpolation)
            });
            if has_interpolation {
                return None;
            }
            let mut value = String::new();
            for f_string in fstring.value.f_strings() {
                for literal in f_string.elements.literals() {
                    value.push_str(&literal.value);
                }
            }
            Some(Cow::Owned(value))
        }
        _ => None,
    }
}

/// ODOO027, ODOO028, ODOO029, ODOO030, ODOO031, ODOO032, ODOO033
pub(crate) fn field_attributes(checker: &Checker, assign: &ast::StmtAssign) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    let Expr::Call(call) = assign.value.as_ref() else {
        return;
    };
    if odoo_field_type(&call.func).is_none() {
        return;
    }

    for keyword in &call.arguments.keywords {
        let Some(arg_name) = keyword.arg.as_ref() else {
            continue;
        };
        let arg_name = arg_name.as_str();

        if matches!(arg_name, "compute" | "search" | "inverse") {
            if let Some(value) = field_attribute_string_value(&keyword.value) {
                let expected_prefix = format!("_{arg_name}_");
                if !value.starts_with(&expected_prefix) {
                    match arg_name {
                        "compute" => {
                            if checker.is_rule_enabled(Rule::MethodCompute) {
                                checker.report_diagnostic(MethodCompute, keyword.value.range());
                            }
                        }
                        "search" => {
                            if checker.is_rule_enabled(Rule::MethodSearch) {
                                checker.report_diagnostic(MethodSearch, keyword.value.range());
                            }
                        }
                        _ => {
                            if checker.is_rule_enabled(Rule::MethodInverse) {
                                checker.report_diagnostic(MethodInverse, keyword.value.range());
                            }
                        }
                    }
                }
            } else if checker.is_rule_enabled(Rule::InheritableMethodString)
                && let Expr::Name(ast::ExprName { id, .. }) = &keyword.value
                && class_defines_method(class_def, id.as_str())
            {
                checker.report_diagnostic(
                    InheritableMethodString {
                        name: id.to_string(),
                    },
                    keyword.value.range(),
                );
            }
        }

        if checker.is_rule_enabled(Rule::InheritableMethodLambda)
            && matches!(arg_name, "default" | "domain")
            && let Expr::Name(ast::ExprName { id, .. }) = &keyword.value
            && class_defines_method(class_def, id.as_str())
        {
            checker.report_diagnostic(
                InheritableMethodLambda {
                    argument: arg_name.to_string(),
                    name: id.to_string(),
                },
                keyword.value.range(),
            );
        }

        if checker.is_rule_enabled(Rule::RenamedFieldParameter)
            && let Some((old, new)) = RENAMED_PARAMETERS.iter().find(|(old, _)| *old == arg_name)
        {
            checker.report_diagnostic(
                RenamedFieldParameter {
                    old: (*old).to_string(),
                    new: (*new).to_string(),
                },
                keyword.range(),
            );
        }
    }

    if checker.is_rule_enabled(Rule::TranslationField) {
        for arg in call.arguments.iter_source_order() {
            let value = arg.value();
            if let Expr::Call(inner) = value
                && matches!(
                    inner.func.as_ref(),
                    Expr::Name(name) if name.id == "_" || name.id == "_lt"
                )
            {
                checker.report_diagnostic(TranslationField, value.range());
            }
        }
    }
}
