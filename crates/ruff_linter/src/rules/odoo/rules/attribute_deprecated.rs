use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for assignments to deprecated old-API attributes (`_columns`, `_defaults`,
/// `length`) on Odoo model classes.
///
/// ## Why is this bad?
/// These attributes belonged to the old Odoo ORM API and have no effect in the current API.
///
/// ## Example
/// ```python
/// class MyModel(models.Model):
///     _inherit = "my.model"
///
///     _defaults = {"active": True}
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct AttributeDeprecated {
    attribute: String,
}

impl Violation for AttributeDeprecated {
    #[derive_message_formats]
    fn message(&self) -> String {
        let AttributeDeprecated { attribute } = self;
        format!("attribute \"{attribute}\" deprecated")
    }
}

const DEPRECATED_ATTRIBUTES: &[&str] = &["_columns", "_defaults", "length"];

/// ODOO021
pub(crate) fn attribute_deprecated(checker: &Checker, assign: &ast::StmtAssign) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    let Some(arguments) = class_def.arguments.as_deref() else {
        return;
    };
    let is_model_class = arguments.args.iter().any(|base| {
        let name = match base {
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str(),
            Expr::Name(ast::ExprName { id, .. }) => id.as_str(),
            _ => return false,
        };
        name.contains("Model")
    });
    if !is_model_class {
        return;
    }

    let [Expr::Name(target)] = assign.targets.as_slice() else {
        return;
    };
    if !DEPRECATED_ATTRIBUTES.contains(&target.id.as_str()) {
        return;
    }
    checker.report_diagnostic(
        AttributeDeprecated {
            attribute: target.id.to_string(),
        },
        assign.range(),
    );
}
