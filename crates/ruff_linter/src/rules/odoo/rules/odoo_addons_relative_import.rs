use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::{Ranged, TextRange};

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
/// manifest file, and returns that directory (the Odoo module's directory), if found.
fn enclosing_odoo_module_dir(path: &Path) -> Option<&Path> {
    let mut dir = path.parent()?;
    loop {
        if std::fs::read_dir(dir).is_ok_and(|mut entries| {
            entries.any(|entry| entry.is_ok_and(|entry| is_manifest_file(&entry.path())))
        }) {
            return Some(dir);
        }
        dir = dir.parent()?;
    }
}

/// Returns `true` for files pylint-odoo's `check_odoo_relative_import` exempted: test files
/// (loaded only when the module and its dependencies are already installed) and migration
/// scripts (versioned, expected to reference the module by its absolute import path).
fn is_exempt_from_relative_import(path: &Path, module_dir: &Path) -> bool {
    let in_tests_dir = path
        .strip_prefix(module_dir)
        .ok()
        .and_then(Path::parent)
        .is_some_and(|parent| parent == Path::new("tests"));
    let in_migrations_dir = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .is_some_and(|name| name == "migrations");
    in_tests_dir || in_migrations_dir
}

/// ODOO023
pub(crate) fn odoo_addons_relative_import(
    checker: &Checker,
    import_from: &ast::StmtImportFrom,
    path: &Path,
) {
    let Some(module_name) = odoo_addons_module_name_from_import_from(import_from) else {
        return;
    };
    check_odoo_addons_relative_import(checker, module_name, path, import_from.range());
}

/// ODOO023
///
/// Unlike `odoo_addons_relative_import`, this covers plain `import odoo.addons.my_module`
/// statements (as opposed to `from odoo.addons.my_module import ...`), which pylint-odoo's
/// `_get_odoo_module_imported` also flags.
pub(crate) fn odoo_addons_relative_import_stmt(
    checker: &Checker,
    stmt: &Stmt,
    names: &[ast::Alias],
    path: &Path,
) {
    for alias in names {
        let Some(module_name) = alias.name.as_str().strip_prefix("odoo.addons.") else {
            continue;
        };
        // `import odoo.addons.my_module.models` imports a submodule of my_module; only the
        // leading segment identifies the Odoo module being imported from.
        let module_name = module_name.split('.').next().unwrap_or(module_name);
        check_odoo_addons_relative_import(checker, module_name, path, stmt.range());
    }
}

/// Extracts the Odoo module name targeted by `from odoo.addons[.my_module[...]] import ...`,
/// mirroring pylint-odoo's `_get_odoo_module_imported`: either the first dotted segment after
/// `odoo.addons.`, or — when the module is imported by name directly off `odoo.addons`, e.g.
/// `from odoo.addons import my_module` — the first imported name.
fn odoo_addons_module_name_from_import_from(import_from: &ast::StmtImportFrom) -> Option<&str> {
    let module = import_from.module.as_deref()?;
    if let Some(rest) = module.strip_prefix("odoo.addons.") {
        return Some(rest.split('.').next().unwrap_or(rest));
    }
    if module == "odoo.addons" {
        return import_from.names.first().map(|alias| alias.name.as_str());
    }
    None
}

fn check_odoo_addons_relative_import(
    checker: &Checker,
    module_name: &str,
    path: &Path,
    range: TextRange,
) {
    let Some(module_dir) = enclosing_odoo_module_dir(path) else {
        return;
    };
    if is_exempt_from_relative_import(path, module_dir) {
        return;
    }
    let Some(enclosing_module) = module_dir.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    if module_name == enclosing_module {
        checker.report_diagnostic(
            OdooAddonsRelativeImport {
                module: enclosing_module.to_string(),
            },
            range,
        );
    }
}
