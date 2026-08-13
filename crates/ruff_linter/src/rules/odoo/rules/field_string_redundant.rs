use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::fix::edits::{Parentheses, remove_argument};
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_field_type};
use crate::{Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for a `string=` argument on an Odoo field definition that's redundant with the
/// field's own variable name.
///
/// ## Why is this bad?
/// Odoo infers a human-readable label from the field's variable name (converting
/// `snake_case` to `Title Case`), so a `string` argument that matches that inferred label
/// is dead weight.
///
/// ## Example
/// ```python
/// class MyModel(models.Model):
///     _inherit = "my.model"
///
///     partner_id = fields.Many2one("res.partner", string="Partner")
/// ```
///
/// Use instead:
/// ```python
/// class MyModel(models.Model):
///     _inherit = "my.model"
///
///     partner_id = fields.Many2one("res.partner")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct FieldStringRedundant;

impl Violation for FieldStringRedundant {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "The `string` attribute is redundant and should be removed".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Remove redundant `string` attribute".to_string())
    }
}

/// The positional index of the `string` argument for field types where it isn't the first
/// positional argument.
fn string_arg_position(field_type: &str) -> usize {
    match field_type {
        "Selection" | "Reference" | "Many2one" => 1,
        "One2many" => 2,
        "Many2many" => 4,
        _ => 0,
    }
}

/// Mimics Python's `str.title()`: uppercase the first letter of each run of alphabetic
/// characters, lowercase the rest.
fn python_title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch.is_alphabetic() {
            if capitalize_next {
                result.extend(ch.to_uppercase());
            } else {
                result.extend(ch.to_lowercase());
            }
            capitalize_next = false;
        } else {
            result.push(ch);
            capitalize_next = true;
        }
    }
    result
}

/// Returns the label Odoo would infer for a field named `field_name`.
fn inferred_label(field_name: &str) -> String {
    let trimmed = field_name
        .strip_suffix("_ids")
        .or_else(|| field_name.strip_suffix("_id"))
        .unwrap_or(field_name);
    python_title_case(&trimmed.replace('_', " "))
}

/// ODOO007
pub(crate) fn field_string_redundant(checker: &Checker, assign: &ast::StmtAssign) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }

    let [Expr::Name(target)] = assign.targets.as_slice() else {
        return;
    };
    let Expr::Call(call) = assign.value.as_ref() else {
        return;
    };
    let Some(field_type) = odoo_field_type(&call.func) else {
        return;
    };

    let mut string_keyword = None;
    for keyword in &call.arguments.keywords {
        let Some(name) = keyword.arg.as_ref() else {
            continue;
        };
        if name == "related" {
            // Related fields inherit their label from the target field; leave them alone.
            return;
        }
        if name == "string" {
            string_keyword = Some(keyword);
        }
    }

    let string_value = if let Some(keyword) = string_keyword {
        let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = &keyword.value else {
            return;
        };
        value.to_str()
    } else {
        let position = string_arg_position(field_type);
        let Some(Expr::StringLiteral(ast::ExprStringLiteral { value, .. })) =
            call.arguments.args.get(position)
        else {
            return;
        };
        value.to_str()
    };

    if string_value != inferred_label(target.id.as_str()) {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(FieldStringRedundant, assign.range());
    diagnostic.try_set_fix(|| {
        let edit = if let Some(keyword) = string_keyword {
            remove_argument(
                keyword,
                &call.arguments,
                Parentheses::Preserve,
                checker.locator().contents(),
                checker.tokens(),
            )
        } else {
            let position = string_arg_position(field_type);
            remove_argument(
                &call.arguments.args[position],
                &call.arguments,
                Parentheses::Preserve,
                checker.locator().contents(),
                checker.tokens(),
            )
        };
        edit.map(Fix::safe_edit)
    });
}
