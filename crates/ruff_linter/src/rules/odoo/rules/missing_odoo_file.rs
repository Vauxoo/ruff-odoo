use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_anchor_range};

/// ## What it does
/// Checks that an Odoo module ships the files the project requires every module to have.
///
/// ## Why is this bad?
/// Projects usually mandate a few per-module files — an icon, a description page, a
/// changelog. A module that silently ships without them is only noticed once it is
/// installed or published.
///
/// ## Options
/// - `lint.odoo.odoo-required-files`
///
/// Each entry is a path relative to the module directory, e.g.
/// `static/description/index.html`. The rule is inert until `odoo-required-files` lists at
/// least one path, mirroring pylint-odoo, whose `--odoo-required-files` also defaults to an
/// empty list. Paid apps have a separate rule, `missing-odoo-file-app` (`OAPP002`).
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.13")]
pub(crate) struct MissingOdooFile {
    path: String,
}

impl Violation for MissingOdooFile {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingOdooFile { path } = self;
        format!("Missing {path} file")
    }
}

/// ODOO066
pub(crate) fn missing_odoo_file(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    let required_files = &checker.settings().odoo.odoo_required_files;
    if required_files.is_empty() {
        return;
    }
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    let Some(dir) = path.parent() else {
        return;
    };
    let module = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".");
    for subpath in required_files {
        if dir.join(subpath).is_file() {
            continue;
        }
        checker.report_diagnostic(
            MissingOdooFile {
                path: format!("{module}/{subpath}"),
            },
            manifest_anchor_range(dict),
        );
    }
}
