use std::borrow::Cow;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_python_stdlib::identifiers::is_identifier;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::rules::odoo::helpers::{class_defines_method, odoo_field_type};
use crate::{Edit, Fix, FixAvailability, Violation};

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
#[violation_metadata(preview_since = "0.16.2.2")]
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
#[violation_metadata(preview_since = "0.16.2.2")]
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
#[violation_metadata(preview_since = "0.16.2.2")]
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
///
/// ## Fix safety
/// This rule's fix is marked as unsafe: the old parameter was silently ignored by the ORM,
/// so renaming it enables behavior (precision, indexing) that the running code did not
/// have. No fix is offered when the call already passes the new parameter, since renaming
/// would produce a duplicate keyword argument.
///
/// ## Options
/// - `lint.odoo.deprecated-field-parameters`
///
/// Each entry is written `old:new`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct RenamedFieldParameter {
    old: String,
    new: String,
}

impl Violation for RenamedFieldParameter {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let RenamedFieldParameter { old, new } = self;
        format!("Field parameter \"{old}\" is no longer supported. Use \"{new}\" instead.")
    }

    fn fix_title(&self) -> Option<String> {
        let RenamedFieldParameter { old, new } = self;
        Some(format!("Replace `{old}` with `{new}`"))
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
///
/// ## Fix safety
/// The fix unwraps the call, keeping its only argument. It is marked as unsafe because the
/// attribute value changes from a string translated at module-load time to the plain
/// string; that is the point of the rule, but code comparing the value against a
/// translated string would change behavior. No fix is offered when the call takes anything
/// but a single positional argument.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct TranslationField;

impl Violation for TranslationField {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Translation method _(\"string\") in fields is not necessary".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Remove the translation call".to_string())
    }
}

/// ## What it does
/// Checks for `compute=`/`search=`/`inverse=` field arguments that pass a direct method
/// reference instead of the method's name as a string.
///
/// ## Why is this bad?
/// A direct reference hardcodes the field to that exact function object at class-definition
/// time. When another module inherits the model and overrides the method, the field keeps
/// calling the original function and the override is silently ignored — the classic Odoo
/// inheritance mechanism simply does not apply. Passing the name as a string makes Odoo
/// resolve the method on the record at runtime, so overrides are honored.
///
/// The direct reference also forces the method to be defined *above* the field definition
/// (otherwise the name is unbound), which is against the convention of declaring fields
/// first and methods after them.
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
///
/// ## Fix safety
/// The fix replaces the reference with the method's name as a string. The rule only fires
/// when the enclosing class defines a method with that name, so the string resolves to a
/// method that exists. The fix is still marked as unsafe because dispatch changes from the
/// bound function object to a name lookup on the record: a subclass override starts being
/// honored (the point of the rule), and if the reference actually pointed at a same-named
/// object from an outer scope — for example an imported function shadowed by a method
/// defined further down the class — the class method replaces it.
///
/// ## References
/// - [OCA/odoo-pre-commit-hooks#126](https://github.com/OCA/odoo-pre-commit-hooks/issues/126),
///   the proposal this rule and [`inheritable-method-lambda`][ODE8148] implement, including
///   which field attributes accept a string and which require a callable.
///
/// [ODE8148]: https://vauxoo.github.io/ruff-odoo/rules/inheritable-method-lambda/
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct InheritableMethodString {
    name: String,
}

impl Violation for InheritableMethodString {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    #[derive_message_formats]
    fn message(&self) -> String {
        let InheritableMethodString { name } = self;
        format!("Use string method name `\"{name}\"` to preserve inheritability")
    }

    fn fix_title(&self) -> Option<String> {
        let InheritableMethodString { name } = self;
        Some(format!("Replace the reference with `\"{name}\"`"))
    }
}

/// ## What it does
/// Checks for `default=`/`domain=` field arguments that pass a direct method reference
/// instead of a lambda.
///
/// ## Why is this bad?
/// A direct reference hardcodes the field to that exact function object at class-definition
/// time. When another module inherits the model and overrides the method, the field keeps
/// calling the original function and the override is silently ignored. Unlike
/// `compute=`/`search=`/`inverse=` (see [`inheritable-method-string`][ODE8147]), these
/// attributes do not accept a method name as a string — Odoo only accepts a plain value or
/// a callable here — so a lambda that dispatches through `self` is what preserves
/// inheritability.
///
/// This is not a theoretical concern: [odoo/odoo#185419] un-hardcoded the `domain=` methods
/// of the Sales Order Item fields for exactly this reason, and [odoo/enterprise#72931] shows
/// the follow-up — once the method became inheritable, an inheriting module's override that
/// had been silently ignored started being called and had to be adapted.
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
///
/// ## References
/// - [OCA/odoo-pre-commit-hooks#126](https://github.com/OCA/odoo-pre-commit-hooks/issues/126),
///   the proposal this rule and [`inheritable-method-string`][ODE8147] implement, including
///   which field attributes accept a string and which require a callable.
///
/// [ODE8147]: https://vauxoo.github.io/ruff-odoo/rules/inheritable-method-string/
/// [odoo/odoo#185419]: https://github.com/odoo/odoo/pull/185419
/// [odoo/enterprise#72931]: https://github.com/odoo/enterprise/pull/72931
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
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

/// ODC8108, ODC8109, ODC8110, ODW8111, ODW8103, ODE8147, ODE8148
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
        let Some(arg_identifier) = keyword.arg.as_ref() else {
            continue;
        };
        let arg_name = arg_identifier.as_str();

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
                let mut diagnostic = checker.report_diagnostic(
                    InheritableMethodString {
                        name: id.to_string(),
                    },
                    keyword.value.range(),
                );
                let quote = checker.stylist().quote();
                diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                    format!("{quote}{id}{quote}"),
                    keyword.value.range(),
                )));
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
            && let Some(new) = checker
                .settings()
                .odoo
                .deprecated_field_parameters
                .renamed(arg_name, RENAMED_PARAMETERS)
        {
            let mut diagnostic = checker.report_diagnostic(
                RenamedFieldParameter {
                    old: arg_name.to_string(),
                    new: new.clone(),
                },
                keyword.range(),
            );
            // Renaming would produce a duplicate keyword argument — a syntax error — if the
            // call already passes the new parameter; a user-configured replacement may also
            // not be a valid identifier at all.
            let conflicts = call
                .arguments
                .keywords
                .iter()
                .any(|other| other.arg.as_deref().is_some_and(|name| name == new));
            if !conflicts && is_identifier(&new) {
                diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                    new,
                    arg_identifier.range(),
                )));
            }
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
                let mut diagnostic = checker.report_diagnostic(TranslationField, value.range());
                if let [term] = &*inner.arguments.args
                    && inner.arguments.keywords.is_empty()
                    && !term.is_starred_expr()
                    && !checker.comment_ranges().intersects(value.range())
                {
                    diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                        checker.locator().slice(term.range()).to_string(),
                        value.range(),
                    )));
                }
            }
        }
    }
}
