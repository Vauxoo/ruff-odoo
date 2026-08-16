use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_anchor_range};

/// ## What it does
/// Checks that an Odoo module has a `README.rst`, `README.md`, or `README.txt` file next to
/// its `__manifest__.py`.
///
/// ## Why is this bad?
/// The README is what's shown on the module's Apps page and is the primary source of
/// documentation for the module.
///
/// ## Options
/// - `lint.odoo.readme-template-url`
///
/// Unset, the diagnostic just reports the missing file; set, it points at the template.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct MissingReadme {
    /// The template a project points contributors at, when it configures one.
    template_url: Option<String>,
}

impl Violation for MissingReadme {
    #[derive_message_formats]
    fn message(&self) -> String {
        match &self.template_url {
            Some(url) => format!("Missing ./README.rst file. Template here: {url}"),
            None => "Missing ./README.rst file".to_string(),
        }
    }
}

const README_FILES: &[&str] = &["README.rst", "README.md", "README.txt"];

/// ODC8112
pub(crate) fn missing_readme(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }
    let Some(dir) = path.parent() else {
        return;
    };
    if README_FILES.iter().any(|name| dir.join(name).is_file()) {
        return;
    }
    checker.report_diagnostic(
        MissingReadme {
            template_url: checker.settings().odoo.readme_template_url.clone(),
        },
        manifest_anchor_range(dict),
    );
}
