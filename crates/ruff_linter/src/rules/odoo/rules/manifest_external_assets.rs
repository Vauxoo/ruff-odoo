use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item};

/// ## What it does
/// Checks for external URLs in the `assets` key of an Odoo module's `__manifest__.py`.
///
/// ## Why is this bad?
/// Assets loaded from a CDN can change or disappear out from under the module, and expose
/// users to third-party outages and supply-chain risks. Assets should be distributed with
/// the module's source code.
///
/// ## Example
/// ```python
/// {
///     "assets": {
///         "web.assets_backend": [
///             "https://cdn.example.com/lib.js",
///         ],
///     },
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct ManifestExternalAssets {
    url: String,
}

impl Violation for ManifestExternalAssets {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestExternalAssets { url } = self;
        format!("Asset {url} should be distributed with module's source code")
    }
}

/// Returns `true` if `url` has a URL scheme (mirroring `urlparse(url).scheme` truthiness).
fn has_url_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// ODOO046
pub(crate) fn manifest_external_assets(
    checker: &Checker,
    dict: &ast::ExprDict,
    path: &std::path::Path,
) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let Some((_, Expr::Dict(assets))) = manifest_item(dict, "assets") else {
        return;
    };
    for item in &assets.items {
        let Expr::List(ast::ExprList { elts, .. }) = &item.value else {
            continue;
        };
        for elt in elts {
            check_asset_element(checker, elt);
        }
    }
}

/// Checks a single element of an assets-key list for an external URL. Directive-style
/// entries (e.g. `("replace", "old/path.js", "https://cdn.example.com/lib.js")`) nest the
/// paths inside a tuple or list, so those are recursed into as well.
fn check_asset_element(checker: &Checker, elt: &Expr) {
    match elt {
        Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
            let url = value.to_str();
            if has_url_scheme(url) {
                checker.report_diagnostic(
                    ManifestExternalAssets {
                        url: url.to_string(),
                    },
                    elt.range(),
                );
            }
        }
        Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
            for nested in elts {
                check_asset_element(checker, nested);
            }
        }
        _ => {}
    }
}
