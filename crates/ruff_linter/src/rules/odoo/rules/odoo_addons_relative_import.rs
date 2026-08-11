use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::is_manifest_file;

/// ## What it does
/// Checks for `odoo.addons.<this_module>` imports, absolute imports of the very module the
/// file itself belongs to.
///
/// ## Why is this bad?
/// A module importing itself by its own name (instead of using a relative import) breaks if
/// the module is ever renamed or vendored under a different name.
///
/// ## Example
/// ```python
/// # in my_module/models/foo.py
/// from odoo.addons.my_module import models
/// ```
///
/// Use instead:
/// ```python
/// from . import models
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct OdooAddonsRelativeImport {
    module: String,
}

impl Violation for OdooAddonsRelativeImport {
    #[derive_message_formats]
    fn message(&self) -> String {
        let OdooAddonsRelativeImport { module } = self;
        format!(
            "Same Odoo module absolute import. You should use relative import with \".\" instead of \"odoo.addons.{module}\""
        )
    }
}

/// Walks up from `path` looking for the nearest ancestor directory containing an Odoo
/// manifest file, and returns that directory's name (the Odoo module name), if found.
fn enclosing_odoo_module_name(path: &Path) -> Option<String> {
    let mut dir = path.parent()?;
    loop {
        if std::fs::read_dir(dir).is_ok_and(|mut entries| {
            entries.any(|entry| entry.is_ok_and(|entry| is_manifest_file(&entry.path())))
        }) {
            return dir.file_name()?.to_str().map(str::to_string);
        }
        dir = dir.parent()?;
    }
}

/// ODOO023
pub(crate) fn odoo_addons_relative_import(
    checker: &Checker,
    import_from: &ast::StmtImportFrom,
    path: &Path,
) {
    let Some(module_name) = import_from
        .module
        .as_deref()
        .and_then(|module| module.strip_prefix("odoo.addons."))
    else {
        return;
    };

    let Some(enclosing_module) = enclosing_odoo_module_name(path) else {
        return;
    };
    if module_name == enclosing_module {
        checker.report_diagnostic(
            OdooAddonsRelativeImport {
                module: enclosing_module,
            },
            import_from.range(),
        );
    }
}
