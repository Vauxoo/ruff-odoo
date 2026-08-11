use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_file, manifest_string_item};

/// ## What it does
/// Checks that the `support` key in an Odoo module's `__manifest__.py`, if present, is a
/// syntactically valid email address.
///
/// ## Why is this bad?
/// Odoo displays this as the module's support contact; an invalid address can't be reached.
///
/// ## Example
/// ```python
/// {
///     "support": "not-an-email",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct InvalidEmail {
    email: String,
}

impl Violation for InvalidEmail {
    #[derive_message_formats]
    fn message(&self) -> String {
        let InvalidEmail { email } = self;
        format!("Invalid email \"{email}\"")
    }
}

/// Simplified structural check, mirroring pylint-odoo's `EMAIL_RE`:
/// `^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$`.
fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    let valid_local = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.+-".contains(c));
    let Some((host, tld)) = domain.rsplit_once('.') else {
        return false;
    };
    let valid_host =
        !host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let valid_tld = !tld.is_empty()
        && tld
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    valid_local && valid_host && valid_tld
}

/// ODOO015
pub(crate) fn invalid_email(checker: &Checker, dict: &ast::ExprDict, path: &std::path::Path) {
    if !is_manifest_file(path) {
        return;
    }
    if !checker.semantic().current_scope().kind.is_module() {
        return;
    }

    let Some((key, email)) = manifest_string_item(dict, "support") else {
        return;
    };
    if email.is_empty() || is_valid_email(email) {
        return;
    }
    checker.report_diagnostic(
        InvalidEmail {
            email: email.to_string(),
        },
        key.range(),
    );
}
