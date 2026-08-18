use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_field_type};

/// ## What it does
/// Checks for a `Many2many` whose second positional argument reads as a label rather than
/// as the name of a table.
///
/// ## Why is this bad?
/// `Many2many` takes `(comodel_name, relation, column1, column2, string)`, so the second
/// positional argument is the name of the relation table, not the label. `Many2one` takes
/// `(comodel_name, string)`, and a field written against that shape puts its label where
/// the table name goes: the label is lost -- Odoo falls back to the one inferred from the
/// field name -- and the table is created under a name nobody meant to give it, quoted
/// capitals and spaces included.
///
/// Only a value that can not be a table name is reported: one carrying capitals or
/// whitespace. Anything that reads like an identifier is taken at its word.
///
/// ## Example
/// ```python
/// class SaleCommissionPlanUserWizard(models.TransientModel):
///     _name = "sale.commission.plan.user.wizard"
///
///     user_ids = fields.Many2many("res.users", "Salespersons")
/// ```
///
/// Use instead:
/// ```python
/// class SaleCommissionPlanUserWizard(models.TransientModel):
///     _name = "sale.commission.plan.user.wizard"
///
///     user_ids = fields.Many2many("res.users", string="Salespersons")
/// ```
///
/// Or name the table as well, which is what Odoo would have generated:
///
/// ```python
/// user_ids = fields.Many2many(
///     "res.users",
///     "sale_commission_plan_user_wizard_res_users_rel",
///     "sale_commission_plan_user_wizard_id",
///     "res_users_id",
///     "Salespersons",
/// )
/// ```
///
/// No fix is offered for either shape. On a database that already ran this code the table
/// exists under the label, so correcting the definition renames it: the rows have to be
/// migrated from the old relation table to the new one, which is a decision for a migration
/// script rather than for a linter.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.25")]
pub(crate) struct M2mRelationIsLabel {
    relation: String,
}

impl Violation for M2mRelationIsLabel {
    #[derive_message_formats]
    fn message(&self) -> String {
        let M2mRelationIsLabel { relation } = self;
        format!(
            "`{relation}` is the relation table of this Many2many, not its label; \
             pass the label as `string=\"{relation}\"`"
        )
    }
}

/// Returns `true` if `value` can not be the name of a table, which is what tells a label
/// apart from an identifier: `PostgreSQL` folds unquoted names to lowercase, and Odoo writes
/// them unquoted, so capitals and whitespace only ever arrive from a label.
fn reads_as_a_label(value: &str) -> bool {
    value
        .chars()
        .any(|char| char.is_uppercase() || char.is_whitespace())
}

/// ODW9501
pub(crate) fn m2m_relation_is_label(checker: &Checker, assign: &ast::StmtAssign) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }
    let Expr::Call(call) = assign.value.as_ref() else {
        return;
    };
    if odoo_field_type(&call.func) != Some("Many2many") {
        return;
    }
    let Some(Expr::StringLiteral(ast::ExprStringLiteral {
        value: relation, ..
    })) = call.arguments.args.get(1)
    else {
        return;
    };
    let relation = relation.to_str();
    if !reads_as_a_label(relation) {
        return;
    }
    checker.report_diagnostic(
        M2mRelationIsLabel {
            relation: relation.to_string(),
        },
        call.arguments.args[1].range(),
    );
}
