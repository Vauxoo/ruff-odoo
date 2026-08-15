use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for `with_context(some_dict)` calls with a single positional argument.
///
/// ## Why is this bad?
/// A single positional dict argument to `with_context` *replaces* the context wholesale
/// instead of merging into it — `with_context(**some_dict)` or `with_context(key=value)` is
/// almost always what's intended.
///
/// ## Example
/// ```python
/// self.with_context(ctx)
/// ```
///
/// Use instead:
/// ```python
/// self.with_context(**ctx)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct ContextOverridden {
    arg: String,
}

impl Violation for ContextOverridden {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ContextOverridden { arg } = self;
        format!(
            "Context overridden using dict. Better using kwargs `with_context(**{arg})` or `with_context(key=value)`"
        )
    }
}

/// ODOO018
pub(crate) fn context_overridden(checker: &Checker, call: &ast::ExprCall) {
    let Expr::Attribute(ast::ExprAttribute { attr, .. }) = call.func.as_ref() else {
        return;
    };
    if attr != "with_context" {
        return;
    }
    if !call.arguments.keywords.is_empty() {
        return;
    }
    let [first, ..] = call.arguments.args.as_ref() else {
        return;
    };
    // `self.with_context(clean_context(self.env.context))` merges a context stripped of
    // request-scoped keys; it's the documented way to reset the context, not an override.
    if let Expr::Call(ast::ExprCall { func, .. }) = first
        && matches!(
            checker
                .semantic()
                .resolve_qualified_name(func)
                .as_ref()
                .map(QualifiedName::segments),
            Some(["odoo", "tools", "clean_context"])
        )
    {
        return;
    }
    checker.report_diagnostic(
        ContextOverridden {
            arg: checker.generator().expr(first),
        },
        call.range(),
    );
}
