use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{MANIFEST_DATA_KEYS, is_manifest_root_dict, manifest_item};

/// ## What it does
/// Checks that every file listed in the data keys (`data`, `demo`, ...) of an Odoo
/// module's `__manifest__.py` exists on disk.
///
/// ## Why is this bad?
/// Odoo aborts the module installation with a `FileNotFoundError` when a listed file is
/// missing, so the mistake only surfaces at install/update time.
///
/// ## Example
/// ```python
/// {
///     "data": [
///         "views/deleted_or_typo_views.xml",
///     ],
/// }
/// ```
///
/// Use instead a path that exists relative to the module:
/// ```python
/// {
///     "data": [
///         "views/res_partner_views.xml",
///     ],
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.5")]
pub(crate) struct ResourceNotExist {
    key: String,
    resource: String,
}

impl Violation for ResourceNotExist {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ResourceNotExist { key, resource } = self;
        format!(r#"File "{key}": "{resource}" not found"#)
    }
}

/// ODF8101
pub(crate) fn resource_not_exist(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    let Some(dir) = path.parent() else {
        return;
    };

    for key in MANIFEST_DATA_KEYS {
        let Some((_, value)) = manifest_item(dict, key) else {
            continue;
        };
        let Expr::List(list) = value else {
            continue;
        };
        for element in &list.elts {
            let Expr::StringLiteral(ast::ExprStringLiteral {
                value: resource, ..
            }) = element
            else {
                continue;
            };
            if dir.join(resource.to_str()).is_file() {
                continue;
            }
            checker.report_diagnostic(
                ResourceNotExist {
                    key: (*key).to_string(),
                    resource: resource.to_str().to_string(),
                },
                element.range(),
            );
        }
    }
}
