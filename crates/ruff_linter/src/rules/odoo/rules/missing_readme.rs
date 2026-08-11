use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::is_manifest_file;

/// ## What it does
/// Checks that an Odoo module has a `README.rst`, `README.md`, or `README.txt` file next to
/// its `__manifest__.py`.
///
/// ## Why is this bad?
/// The README is what's shown on the module's Apps page and is the primary source of
/// documentation for the module.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct MissingReadme;

impl Violation for MissingReadme {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Missing ./README.rst file".to_string()
    }
}

const README_FILES: &[&str] = &["README.rst", "README.md", "README.txt"];

/// ODOO016
pub(crate) fn missing_readme(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_file(path) {
        return;
    }
    if !checker.semantic().current_scope().kind.is_module() {
        return;
    }
    let Some(dir) = path.parent() else {
        return;
    };
    if README_FILES.iter().any(|name| dir.join(name).is_file()) {
        return;
    }
    checker.report_diagnostic(MissingReadme, dict.range());
}
