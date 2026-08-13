use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, remove_dict_item};
use crate::{Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for deprecated keys in an Odoo module's `__manifest__.py`.
///
/// ## Why is this bad?
/// The `description` manifest key has been superseded by the module's
/// `README.rst`/`README.md` file, and is ignored by modern versions of Odoo.
///
/// ## Example
/// ```python
/// {
///     "name": "My Module",
///     "description": "Does things.",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "name": "My Module",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ManifestDeprecatedKey {
    key: &'static str,
}

impl Violation for ManifestDeprecatedKey {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestDeprecatedKey { key } = self;
        format!("Deprecated key \"{key}\" in manifest file")
    }

    fn fix_title(&self) -> Option<String> {
        let ManifestDeprecatedKey { key } = self;
        Some(format!("Remove deprecated key \"{key}\""))
    }
}

/// ODOO002
pub(crate) fn manifest_deprecated_key(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, path) {
        return;
    }

    for item in &dict.items {
        let Some(key_expr @ ast::Expr::StringLiteral(ast::ExprStringLiteral { value, .. })) =
            &item.key
        else {
            continue;
        };
        if value.to_str() != "description" {
            continue;
        }

        // Report on the key: the deprecated `description` value is typically a large
        // multi-line string and highlighting all of it drowns the editor.
        let mut diagnostic = checker.report_diagnostic(
            ManifestDeprecatedKey { key: "description" },
            key_expr.range(),
        );
        diagnostic.try_set_fix(|| {
            remove_dict_item(dict, item, checker.locator().contents()).map(Fix::safe_edit)
        });
    }
}
