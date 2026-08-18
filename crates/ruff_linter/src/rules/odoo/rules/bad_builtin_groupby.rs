use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::importer::ImportRequest;
use crate::{Edit, Fix, FixAvailability, Violation};

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
///
/// ## Fix safety
/// The fix rewrites the call to `odoo.tools.groupby`, importing it if necessary. It is
/// marked as unsafe because the two functions are not interchangeable: `odoo.tools.groupby`
/// sorts the whole input first and returns a list of `(key, list)` pairs, where
/// `itertools.groupby` lazily yields one group per *consecutive* run. Code that relied on
/// the consecutive-run behavior, or on the groups being iterators, changes meaning.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct BadBuiltinGroupby;

impl Violation for BadBuiltinGroupby {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Used builtin function `itertools.groupby`. Prefer `odoo.tools.groupby` instead."
            .to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Replace with `odoo.tools.groupby`".to_string())
    }
}

/// ODW8155
pub(crate) fn bad_builtin_groupby(checker: &Checker, call: &ast::ExprCall) {
    if checker
        .semantic()
        .resolve_qualified_name(&call.func)
        .is_some_and(|name| matches!(name.segments(), ["itertools", "groupby"]))
    {
        let mut diagnostic = checker.report_diagnostic(BadBuiltinGroupby, call.range());
        diagnostic.try_set_fix(|| {
            let (import_edit, binding) = checker.importer().get_or_import_symbol(
                &ImportRequest::import_from("odoo.tools", "groupby"),
                call.start(),
                checker.semantic(),
            )?;
            Ok(Fix::unsafe_edits(
                Edit::range_replacement(binding, call.func.range()),
                [import_edit],
            ))
        });
    }
}
