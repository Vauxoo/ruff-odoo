use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;
use rustc_hash::FxHashSet;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{
    MANIFEST_DATA_KEYS, is_manifest_root_dict, manifest_item, remove_list_element,
};
use crate::{AlwaysFixableViolation, Fix};

/// ## What it does
/// Checks for file paths listed more than once in the same data key (`data`, `demo`, ...)
/// of an Odoo module's `__manifest__.py`.
///
/// ## Why is this bad?
/// Odoo loads every listed file in order, so a duplicated entry is loaded twice: records
/// are re-created or re-written and module installation gets slower for no reason.
///
/// ## Example
/// ```python
/// {
///     "data": [
///         "views/res_partner_views.xml",
///         "views/res_partner_views.xml",
///     ],
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "data": [
///         "views/res_partner_views.xml",
///     ],
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ManifestDataDuplicated {
    file: String,
    key: String,
}

impl AlwaysFixableViolation for ManifestDataDuplicated {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestDataDuplicated { file, key } = self;
        format!(r#"The file "{file}" is duplicated in manifest key "{key}""#)
    }

    fn fix_title(&self) -> String {
        "Remove the duplicated entry".to_string()
    }
}

/// ODOO048
pub(crate) fn manifest_data_duplicated(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, path) {
        return;
    }

    for key in MANIFEST_DATA_KEYS {
        let Some((_, value)) = manifest_item(dict, key) else {
            continue;
        };
        let Expr::List(list) = value else {
            continue;
        };
        let mut seen: FxHashSet<&str> = FxHashSet::default();
        for element in &list.elts {
            let Expr::StringLiteral(ast::ExprStringLiteral { value: file, .. }) = element else {
                continue;
            };
            if seen.insert(file.to_str()) {
                continue;
            }
            let mut diagnostic = checker.report_diagnostic(
                ManifestDataDuplicated {
                    file: file.to_str().to_string(),
                    key: (*key).to_string(),
                },
                element.range(),
            );
            diagnostic.try_set_fix(|| {
                remove_list_element(list, element, checker.locator().contents()).map(Fix::safe_edit)
            });
        }
    }
}
