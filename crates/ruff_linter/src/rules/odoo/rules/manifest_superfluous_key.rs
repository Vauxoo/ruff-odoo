use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, remove_dict_item};
use crate::{Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for manifest keys explicitly set to their own default value.
///
/// ## Why is this bad?
/// Setting a key to the value it would already have by default is redundant.
///
/// ## Example
/// ```python
/// {
///     "installable": True,
///     "depends": [],
/// }
/// ```
///
/// Use instead:
///
/// ```python
/// {}
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ManifestSuperfluousKey {
    key: String,
}

impl Violation for ManifestSuperfluousKey {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestSuperfluousKey { key } = self;
        format!("Manifest superfluous key \"{key}\". It is the same as the default value")
    }

    fn fix_title(&self) -> Option<String> {
        let ManifestSuperfluousKey { key } = self;
        Some(format!("Remove superfluous key \"{key}\""))
    }
}

/// Keys whose Odoo-assigned default is `True`; every other key defaults to a falsy value
/// (`False`, `""`, `[]`, `{}`, `0`, or `None`).
const DEFAULT_TRUE_KEYS: &[&str] = &["active", "installable"];

fn is_default_value(key: &str, value: &Expr) -> bool {
    let defaults_true = DEFAULT_TRUE_KEYS.contains(&key);
    match value {
        Expr::BooleanLiteral(ast::ExprBooleanLiteral { value, .. }) => *value == defaults_true,
        Expr::NoneLiteral(_) => !defaults_true,
        Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
            !defaults_true && value.to_str().is_empty()
        }
        Expr::List(ast::ExprList { elts, .. }) => !defaults_true && elts.is_empty(),
        Expr::Dict(ast::ExprDict { items, .. }) => !defaults_true && items.is_empty(),
        Expr::NumberLiteral(ast::ExprNumberLiteral {
            value: ast::Number::Int(int),
            ..
        }) => !defaults_true && *int == ast::Int::ZERO,
        _ => false,
    }
}

/// ODOO025
pub(crate) fn manifest_superfluous_key(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, path) {
        return;
    }

    for item in &dict.items {
        let Some(key_expr @ Expr::StringLiteral(ast::ExprStringLiteral { value: key, .. })) =
            &item.key
        else {
            continue;
        };
        let key = key.to_str();
        if !is_default_value(key, &item.value) {
            continue;
        }

        // Report on the key: the superfluous value can span several lines (lists, dicts)
        // and highlighting all of it drowns the editor.
        let mut diagnostic = checker.report_diagnostic(
            ManifestSuperfluousKey {
                key: key.to_string(),
            },
            key_expr.range(),
        );
        diagnostic.try_set_fix(|| {
            remove_dict_item(dict, item, checker.locator().contents()).map(Fix::safe_edit)
        });
    }
}
