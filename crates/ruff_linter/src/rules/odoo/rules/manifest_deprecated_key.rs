use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, odoo_version_applies, remove_dict_item};
use crate::rules::odoo::settings::{ManifestDeprecatedKeys, OdooVersion};
use crate::{Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for deprecated keys in an Odoo module's `__manifest__.py`.
///
/// ## Why is this bad?
/// A deprecated key is ignored by the versions of Odoo that deprecated it, so it silently
/// stops doing what its author expected: `description` has been superseded by the module's
/// `README.rst`/`README.md`, `active` by `auto_install`, and the `qweb` key by the `assets`
/// key since Odoo 16.0 removed the `web.assets_qweb` bundle.
///
/// ## Example
/// ```python
/// {
///     "name": "My Module",
///     "description": "Does things.",
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "name": "My Module",
/// }
/// ```
///
/// ## Options
/// - `lint.odoo.manifest-deprecated-keys`
/// - `lint.odoo.odoo-version`
///
/// By default the rule reports `active`, `description` and — from Odoo 16.0 on, according to
/// the configured [`odoo-version`](../settings.md#lint_odoo_odoo-version) — `qweb`. That is
/// wider than pylint-odoo, whose `--manifest-deprecated-keys` defaults to `description` alone
/// and has no notion of the version a key was deprecated in, so projects had to spell the
/// other keys out in their configuration. Setting
/// [`manifest-deprecated-keys`](../settings.md#lint_odoo_manifest-deprecated-keys) replaces
/// that built-in list, and the keys it names are then reported for every Odoo version.
///
/// ## Fix safety
/// The fix that removes the key is marked safe only for `description`, which modern Odoo
/// ignores outright. For every other key it is unsafe: `active` and `qweb` may still need
/// their replacement (`auto_install`, an `assets` entry) rather than plain deletion, and a
/// key named through `manifest-deprecated-keys` can mean anything at all.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct ManifestDeprecatedKey {
    key: String,
}

impl Violation for ManifestDeprecatedKey {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let ManifestDeprecatedKey { key } = self;
        format!("Deprecated key \"{key}\" in manifest file")
    }

    fn fix_title(&self) -> Option<String> {
        let ManifestDeprecatedKey { key } = self;
        Some(format!("Remove deprecated key \"{key}\""))
    }
}

/// A manifest key Odoo deprecated, and the version it was deprecated in.
struct DeprecatedKey {
    name: &'static str,
    /// The first Odoo version that deprecated the key, or `None` for a key that has been
    /// deprecated for as long as any supported version.
    since: Option<OdooVersion>,
}

/// The keys reported when `manifest-deprecated-keys` is left at its default.
const DEPRECATED_KEYS: &[DeprecatedKey] = &[
    // Renamed to `auto_install` long before the oldest version this fork targets.
    DeprecatedKey {
        name: "active",
        since: None,
    },
    // Superseded by the module's README.
    DeprecatedKey {
        name: "description",
        since: None,
    },
    // 16.0 dropped the `web.assets_qweb` bundle: those templates are declared in `assets`.
    DeprecatedKey {
        name: "qweb",
        since: Some(OdooVersion::new(16, 0)),
    },
];

/// ODC8103
pub(crate) fn manifest_deprecated_key(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    for item in &dict.items {
        let Some(key_expr @ ast::Expr::StringLiteral(ast::ExprStringLiteral { value, .. })) =
            &item.key
        else {
            continue;
        };
        let key = value.to_str();
        if !is_deprecated(checker, key) {
            continue;
        }

        // Report on the key: a deprecated value such as `description` is typically a large
        // multi-line string and highlighting all of it drowns the editor.
        let mut diagnostic = checker.report_diagnostic(
            ManifestDeprecatedKey {
                key: key.to_string(),
            },
            key_expr.range(),
        );
        diagnostic.try_set_fix(|| {
            let edit = remove_dict_item(dict, item, checker.locator().contents())?;
            Ok(if key == "description" {
                Fix::safe_edit(edit)
            } else {
                Fix::unsafe_edit(edit)
            })
        });
    }
}

/// Returns `true` if `key` is deprecated for the configured Odoo version.
fn is_deprecated(checker: &Checker, key: &str) -> bool {
    match &checker.settings().odoo.manifest_deprecated_keys {
        ManifestDeprecatedKeys::Default => DEPRECATED_KEYS.iter().any(|deprecated| {
            deprecated.name == key && odoo_version_applies(checker, deprecated.since, None)
        }),
        ManifestDeprecatedKeys::UserProvided(keys) => keys.iter().any(|entry| entry == key),
    }
}
