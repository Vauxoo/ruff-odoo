use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::is_manifest_file;
use crate::{Edit, Fix, FixAvailability, Violation};

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
///
/// ## Fix safety
/// For a `from odoo.addons.<module>... import ...` statement the fix replaces the dotted
/// path with the equivalent relative one, computed from the file's location inside the
/// module; the imported module is the same, so the fix is safe. The other spellings —
/// `import odoo.addons.<module>` and `from odoo.addons import <module>` — bind the
/// absolute name and have no relative equivalent, so they get no fix.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct OdooAddonsRelativeImport {
    module: String,
}

impl Violation for OdooAddonsRelativeImport {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let OdooAddonsRelativeImport { module } = self;
        format!(
            "Same Odoo module absolute import. You should use relative import with \".\" instead of \"odoo.addons.{module}\""
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some("Use a relative import".to_string())
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

/// ODW8150
pub(crate) fn odoo_addons_relative_import(
    checker: &Checker,
    import_from: &ast::StmtImportFrom,
    path: &Path,
) {
    let Some(module_name) = odoo_addons_module_name_from_import_from(import_from) else {
        return;
    };
    let Some(module_dir) = same_module_dir(module_name, path) else {
        return;
    };
    let mut diagnostic = checker.report_diagnostic(
        OdooAddonsRelativeImport {
            module: module_name.to_string(),
        },
        import_from.range(),
    );

    // Only `from odoo.addons.<module>... import ...` has a relative equivalent; the
    // `from odoo.addons import <module>` spelling binds the module by its absolute name.
    let Some(module_ident) = import_from.module.as_ref() else {
        return;
    };
    let Some(rest) = module_ident
        .as_str()
        .strip_prefix("odoo.addons.")
        .and_then(|rest| rest.strip_prefix(module_name))
    else {
        return;
    };
    if let Some(relative) = relative_module_path(path, module_dir, rest.trim_start_matches('.')) {
        diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
            relative,
            module_ident.range(),
        )));
    }
}

/// Spells `target_subpath` (the dotted path inside the module, possibly empty) relative to
/// the package `path` lives in: from `my_module/models/foo.py`, `models.bar` is `.bar` and
/// `wizards.baz` is `..wizards.baz`.
fn relative_module_path(path: &Path, module_dir: &Path, target_subpath: &str) -> Option<String> {
    let package_depth: Vec<&str> = path
        .parent()?
        .strip_prefix(module_dir)
        .ok()?
        .iter()
        .map(|component| component.to_str())
        .collect::<Option<Vec<_>>>()?;
    let target: Vec<&str> = if target_subpath.is_empty() {
        Vec::new()
    } else {
        target_subpath.split('.').collect()
    };
    let shared = package_depth
        .iter()
        .zip(&target)
        .take_while(|(package, target)| package == target)
        .count();
    let dots = ".".repeat(package_depth.len() - shared + 1);
    Some(format!("{dots}{}", target[shared..].join(".")))
}

/// ODW8150
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
        if same_module_dir(module_name, path).is_some() {
            checker.report_diagnostic(
                OdooAddonsRelativeImport {
                    module: module_name.to_string(),
                },
                stmt.range(),
            );
        }
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

/// The enclosing module directory, when `module_name` names the very module `path` lives
/// in and the file is not exempt.
fn same_module_dir<'a>(module_name: &str, path: &'a Path) -> Option<&'a Path> {
    let module_dir = enclosing_odoo_module_dir(path)?;
    if is_exempt_from_relative_import(path, module_dir) {
        return None;
    }
    let enclosing_module = module_dir.file_name().and_then(|name| name.to_str())?;
    (module_name == enclosing_module).then_some(module_dir)
}
