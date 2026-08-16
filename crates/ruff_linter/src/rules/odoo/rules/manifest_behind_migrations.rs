use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_string_item};

/// ## What it does
/// Checks that the `version` in an Odoo module's `__manifest__.py` is not lower than the
/// version of any migration script directory under the module's `migrations/` folder.
///
/// ## Why is this bad?
/// Odoo only runs a migration script when the module is upgraded to a version greater than
/// or equal to the script's version. If the manifest version stays behind the migration
/// script's version, the migration script never runs.
///
/// ## Example
/// ```python
/// # migrations/18.0.1.0.1/pre-migration.py exists
/// {
///     "name": "My Module",
///     "version": "18.0.1.0.0",
/// }
/// ```
///
/// Use instead:
/// ```python
/// # migrations/18.0.1.0.1/pre-migration.py exists
/// {
///     "name": "My Module",
///     "version": "18.0.1.0.1",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.8")]
pub(crate) struct ManifestBehindMigrations {
    version: String,
    migration: String,
}

impl Violation for ManifestBehindMigrations {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestBehindMigrations { version, migration } = self;
        format!("Manifest version ({version}) is lower than migration scripts ({migration})")
    }
}

/// Parses a version string of integers separated by dots (e.g. `18.0.1.0.0`); returns
/// `None` for anything else, mirroring pylint-odoo's `version2tuple`.
fn version_tuple(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// ODE8145
pub(crate) fn manifest_behind_migrations(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    let Some((key, version)) = manifest_string_item(dict, "version") else {
        return;
    };
    let Some(manifest_version) = version_tuple(version) else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir.join("migrations")) else {
        return;
    };
    // Report only the highest migration version ahead of the manifest; the fix (bumping the
    // manifest version to at least that one) covers every lower script at once.
    let behind = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let parsed = version_tuple(&name)?;
            (parsed > manifest_version).then_some((parsed, name))
        })
        .max();
    if let Some((_parsed, migration)) = behind {
        checker.report_diagnostic(
            ManifestBehindMigrations {
                version: version.to_string(),
                migration,
            },
            key.range(),
        );
    }
}
