use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_anchor_range, manifest_item};

/// ## What it does
/// Checks that an Odoo module whose `__manifest__.py` has a `price` key (a paid app) ships
/// the files the Odoo Apps store requires, such as `static/description/index.html`.
///
/// ## Why is this bad?
/// Paid apps are published on [apps.odoo.com](https://apps.odoo.com/apps), which renders
/// `static/description/index.html` as the app's store page. Without it the app is listed
/// with no description page.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct MissingOdooFileApp {
    path: String,
}

impl Violation for MissingOdooFileApp {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingOdooFileApp { path } = self;
        format!("Missing {path} file for modules with price")
    }
}

const REQUIRED_FILES_APP: &[&str] = &["static/description/index.html"];

/// ODOOAPP002
pub(crate) fn missing_odoo_file_app(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, path) {
        return;
    }
    if manifest_item(dict, "price").is_none() {
        return;
    }
    let Some(dir) = path.parent() else {
        return;
    };
    let module = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".");
    for subpath in REQUIRED_FILES_APP {
        if dir.join(subpath).is_file() {
            continue;
        }
        checker.report_diagnostic(
            MissingOdooFileApp {
                path: format!("{module}/{subpath}"),
            },
            manifest_anchor_range(dict),
        );
    }
}
