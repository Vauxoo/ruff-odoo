use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for the `inselect`/`not inselect` domain operators.
///
/// ## Why is this bad?
/// These operators were removed in Odoo 18.0 in favor of `in`/`not in` with an SQL
/// subquery value.
///
/// ## Example
/// ```python
/// domain = [("id", "inselect", (query, params))]
/// ```
///
/// Use instead:
/// ```python
/// domain = [("id", "in", SQL(...))]
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct DeprecatedInselectOperator {
    operator: String,
}

impl Violation for DeprecatedInselectOperator {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DeprecatedInselectOperator { operator } = self;
        format!(
            "The domain operator '{operator}' is deprecated in Odoo 18.0+. Use 'in' SQL or 'not in' SQL instead"
        )
    }
}

/// ODOO044
pub(crate) fn deprecated_inselect_operator(checker: &Checker, string: &ast::ExprStringLiteral) {
    // `inselect`/`not inselect` were only removed in Odoo 18.0.
    if !odoo_version_applies(checker, Some(OdooVersion::new(18, 0)), None) {
        return;
    }
    let text = string.value.to_str();
    if text.eq_ignore_ascii_case("inselect") || text.eq_ignore_ascii_case("not inselect") {
        checker.report_diagnostic(
            DeprecatedInselectOperator {
                operator: text.to_string(),
            },
            string.range(),
        );
    }
}
