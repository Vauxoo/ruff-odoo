use std::sync::LazyLock;

use regex::Regex;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_string_item};

/// Mirrors pylint-odoo's `DOMAIN_RE`: one or more dot-separated labels (letters, digits,
/// internal hyphens), followed by a final label that may contain underscores but must end in a
/// letter.
static DOMAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9][a-z0-9_-]{0,61}[a-z]$")
        .unwrap()
});

/// ## What it does
/// Checks that the `website` key in an Odoo module's `__manifest__.py`, if present, is a
/// valid `http(s)://` URI.
///
/// ## Why is this bad?
/// Odoo displays this URL on the module's Apps page; an invalid one is a dead link.
///
/// ## Example
/// ```python
/// {
///     "website": "example.com",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "website": "https://example.com",
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct WebsiteManifestKeyNotValidUri {
    website: String,
}

impl Violation for WebsiteManifestKeyNotValidUri {
    #[derive_message_formats]
    fn message(&self) -> String {
        let WebsiteManifestKeyNotValidUri { website } = self;
        format!("Website \"{website}\" in manifest key is not a valid URI")
    }
}

/// A simplified structural check: requires an `http(s)://` scheme, a non-empty domain with
/// no internal whitespace, and no double-underscore in the domain (a common spoofing trick).
fn is_valid_website_uri(url: &str) -> bool {
    if url.is_empty() || url.chars().any(char::is_whitespace) {
        return false;
    }
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let netloc = rest.split(['/', '?', '#']).next().unwrap_or("");
    !netloc.is_empty() && !netloc.contains("__") && DOMAIN_RE.is_match(netloc)
}

/// ODOO014
pub(crate) fn website_manifest_key_not_valid_uri(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let Some((key, website)) = manifest_string_item(dict, "website") else {
        return;
    };
    if website.is_empty() || is_valid_website_uri(website) {
        return;
    }
    checker.report_diagnostic(
        WebsiteManifestKeyNotValidUri {
            website: website.to_string(),
        },
        key.range(),
    );
}
