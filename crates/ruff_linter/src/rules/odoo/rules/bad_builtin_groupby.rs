use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks for use of `itertools.groupby`.
///
/// ## Why is this bad?
/// `itertools.groupby` only groups *consecutive* runs, which is a frequent footgun. Prefer
/// `odoo.tools.groupby`, which sorts first.
///
/// ## Example
/// ```python
/// import itertools
///
/// itertools.groupby(records, key=lambda r: r.partner_id)
/// ```
///
/// Use instead:
/// ```python
/// from odoo.tools import groupby
///
/// groupby(records, key=lambda r: r.partner_id)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct BadBuiltinGroupby;

impl Violation for BadBuiltinGroupby {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Used builtin function `itertools.groupby`. Prefer `odoo.tools.groupby` instead."
            .to_string()
    }
}

/// ODW8155
pub(crate) fn bad_builtin_groupby(checker: &Checker, call: &ast::ExprCall) {
    if checker
        .semantic()
        .resolve_qualified_name(&call.func)
        .is_some_and(|name| matches!(name.segments(), ["itertools", "groupby"]))
    {
        checker.report_diagnostic(BadBuiltinGroupby, call.range());
    }
}
