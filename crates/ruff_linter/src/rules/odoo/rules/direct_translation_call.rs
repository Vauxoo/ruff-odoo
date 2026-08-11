use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for calls to the bare `_()`/`_lt()` translation functions.
///
/// ## Why is this bad?
/// Since Odoo 18.0, `self.env._()` is preferred over the bare `_()`/`_lt()` functions.
///
/// ## Example
/// ```python
/// def my_method(self):
///     return _("Hello")
/// ```
///
/// Use instead:
/// ```python
/// def my_method(self):
///     return self.env._("Hello")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DirectTranslationCall;

impl Violation for DirectTranslationCall {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Better using self.env._".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Replace with `self.env._`".to_string())
    }
}

/// ODOO024
pub(crate) fn direct_translation_call(checker: &Checker, call: &ast::ExprCall) {
    let Expr::Name(ast::ExprName { id, .. }) = call.func.as_ref() else {
        return;
    };
    if id != "_" && id != "_lt" {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(DirectTranslationCall, call.func.range());

    // Only offer the fix inside a method whose first parameter is `self` — outside of one,
    // `self.env` wouldn't resolve to anything.
    let ScopeKind::Function(function_def) = checker.semantic().current_scope().kind else {
        return;
    };
    let Some(first_param) = function_def.parameters.args.first() else {
        return;
    };
    if first_param.parameter.name.as_str() != "self" {
        return;
    }
    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        "self.env._".to_string(),
        call.func.range(),
    )));
}
