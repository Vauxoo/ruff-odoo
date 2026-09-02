use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability};

/// ## What it does
/// Checks for calls to ORM methods Odoo has deprecated, according to the configured
/// [`odoo-version`](../settings.md#lint_odoo_odoo-version).
///
/// ## Why is this bad?
/// A deprecated method raises a `DeprecationWarning` at runtime and will eventually be
/// removed. Most of the replacements are not drop-in — signatures, return types, and in the
/// case of `_check_recursion` even the sense of the returned boolean differ — so the call
/// sites have to be reviewed by hand.
///
/// Odoo dropped most of these after 19.0, so on a version at or past the removal the message
/// says the call raises `AttributeError` rather than warning.
///
/// ## Example
/// ```python
/// groups = self.read_group(domain, ["amount:sum"], ["partner_id"])
/// ```
///
/// Use instead:
/// ```python
/// groups = self._read_group(domain, ["partner_id"], ["amount:sum"])
/// ```
///
/// ## References
/// - [`read_group` on 19.0][read-group-19] — the last release that ships the deprecated one.
/// - [`read_group` on 20.0][read-group-20] — the name reused for an RPC adapter over
///   `_read_group`, taking `(domain, groupby, aggregates, ...)` and returning ids.
/// - [`_read_group`][read-group-private] — unchanged since 16.0, and what backend code wants.
/// - [`check_access_rights` on 19.0][check-access-rights] — the access methods, still present.
/// - [`_check_access` on 20.0][check-access-20] — itself deprecated there, for `_access_domain`.
///
/// [read-group-19]: https://github.com/odoo/odoo/blob/457684cc3377cda5167a4002aa1816b4aa15699f/odoo/orm/models.py#L2752-L2893
/// [read-group-20]: https://github.com/odoo/odoo/blob/928ae2ba164022a51cdfe548dec9491c61339a5f/odoo/orm/models.py#L1915-L1978
/// [read-group-private]: https://github.com/odoo/odoo/blob/928ae2ba164022a51cdfe548dec9491c61339a5f/odoo/orm/models.py#L1980-L2070
/// [check-access-rights]: https://github.com/odoo/odoo/blob/457684cc3377cda5167a4002aa1816b4aa15699f/odoo/orm/models.py#L4166-L4179
/// [check-access-20]: https://github.com/odoo/odoo/blob/928ae2ba164022a51cdfe548dec9491c61339a5f/odoo/orm/models.py#L3549-L3573
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.14")]
pub(crate) struct DeprecatedOdooMethodCall {
    name: String,
    since: OdooVersion,
    /// The version that dropped the method, set only when the configured `odoo-version` is
    /// already at or past it, so that the message can say "removed" instead of "deprecated".
    removed: Option<OdooVersion>,
    /// The version that reused the name for a different method, set only when the configured
    /// `odoo-version` is already at or past it. Only `read_group` has one.
    replaced: Option<OdooVersion>,
    /// The keyword that gave the call away as the pre-replacement API.
    legacy_keyword: Option<String>,
    replacement: String,
    rename: Option<String>,
}

impl Violation for DeprecatedOdooMethodCall {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let DeprecatedOdooMethodCall {
            name,
            since,
            removed,
            replaced,
            legacy_keyword,
            replacement,
            ..
        } = self;
        match (removed, replaced, legacy_keyword) {
            (_, Some(replaced), Some(keyword)) => format!(
                "`{name}` was replaced in Odoo {replaced}: the name is back as the RPC adapter \
                 over `_read_group`, whose signature has no `{keyword}`, so this is still the \
                 pre-{replaced} API. {replacement}"
            ),
            (Some(removed), _, _) => format!(
                "`{name}` was removed in Odoo {removed} (deprecated since {since}). {replacement}"
            ),
            _ => format!("`{name}` is deprecated since Odoo {since}. {replacement}"),
        }
    }

    fn fix_title(&self) -> Option<String> {
        let rename = self.rename.as_ref()?;
        Some(format!("Replace with `{rename}`"))
    }
}

/// An ORM method deprecated by Odoo, and how to move off it.
struct DeprecatedMethod {
    /// The deprecated method name.
    name: &'static str,
    /// The Odoo version that deprecated it.
    since: OdooVersion,
    /// The Odoo version that deleted the method outright. Past it the call raises
    /// `AttributeError` rather than warning, which the message says instead.
    removed: Option<OdooVersion>,
    /// The last Odoo version the deprecation applies to.
    until: Option<OdooVersion>,
    /// The version that reused the name for a different method. From there on the call is only
    /// reported when it carries one of `legacy_keywords`, which no current signature accepts —
    /// anything else is indistinguishable from a correct use of the new method.
    reintroduced: Option<OdooVersion>,
    /// Keywords only the pre-`reintroduced` signature ever accepted.
    legacy_keywords: &'static [&'static str],
    /// Prose naming the replacement, appended to the diagnostic message.
    replacement: &'static str,
    /// The new method name, set only where the deprecated method is a pure one-line
    /// delegation with the same signature, so that swapping the name is enough. Emitted as an
    /// unsafe fix, because the rule cannot prove the receiver really is a recordset.
    rename: Option<&'static str>,
}

/// The release that deleted the access and cycle methods deprecated in 18.0 and 19.0 —
/// commits `0915e817` and `37ba3b16`, both landed on 2025-09-11, after 19.0 was cut.
///
/// Odoo shipped them in a `saas~19.x` branch first, but the `odoo-version` option takes the
/// stable series a project actually runs, so every version this table names is an `x.0`.
/// A `saas~19.x` has no stable release of its own and naming one here would leave a project
/// unable to spell its version at all.
const REMOVED_AFTER_ODOO_19: Option<OdooVersion> = Some(OdooVersion::new(20, 0));

/// Methods marked `@api.deprecated` in Odoo's `odoo/orm/models.py`, up to 19.0.
///
/// From 20.0 that decorator is gone from the file: the deprecations there are marked with
/// `odoo.tools.func.deprecated` instead, so a newer entry has to be read from that list.
///
/// Only the core ORM is covered: addon-specific deprecations (`ir.cron._notify_progress`,
/// `ir.http.get_currencies`, ...) are left out, since their names are not distinctive enough
/// to match on a receiver of unknown model.
const DEPRECATED_ORM_METHODS: &[DeprecatedMethod] = &[
    DeprecatedMethod {
        name: "check_access_rights",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        // Not a rename: `raise_exception=False` has to become `has_access`, and the deprecated
        // method checks the model through `self.browse()` rather than the records in `self`.
        replacement: "Use `check_access` instead, or `has_access` where a boolean is needed; \
                      an override belongs in `_check_access`, or `_access_domain` from 20.0.",
        rename: None,
    },
    DeprecatedMethod {
        name: "check_access_rule",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        replacement: "Use `check_access` instead; an override belongs in `_check_access`, \
                      or `_access_domain` from 20.0.",
        rename: Some("check_access"),
    },
    DeprecatedMethod {
        name: "_filter_access_rules",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        replacement: "Use `_filtered_access` instead; an override belongs in `_check_access`, \
                      or `_access_domain` from 20.0.",
        rename: Some("_filtered_access"),
    },
    DeprecatedMethod {
        name: "_filter_access_rules_python",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        replacement: "Use `_filtered_access` instead; an override belongs in `_check_access`, \
                      or `_access_domain` from 20.0.",
        rename: Some("_filtered_access"),
    },
    DeprecatedMethod {
        name: "_check_recursion",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        // Not a rename: `_check_recursion` returns `not self._has_cycle(...)`, so the caller
        // has to flip the condition too.
        replacement: "Use `not _has_cycle(...)` instead — the result is inverted.",
        rename: None,
    },
    DeprecatedMethod {
        name: "_check_m2m_recursion",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        replacement: "Use `not _has_cycle(...)` instead — the result is inverted.",
        rename: None,
    },
    DeprecatedMethod {
        name: "read_group",
        since: OdooVersion::new(19, 0),
        // Deleted alongside the others, then 20.0 reused the name for a different method: an
        // RPC adapter over `_read_group` taking `(domain, groupby, aggregates, ...)` and
        // returning ids, where the old one took `(domain, fields, groupby, ..., lazy)` and
        // returned dicts. So it is not a removal, and it is not correct either — from 20.0 the
        // call is only reported when a keyword gives away the old API.
        removed: None,
        until: None,
        reintroduced: Some(OdooVersion::new(20, 0)),
        legacy_keywords: &["lazy", "orderby"],
        replacement: "Use `_read_group` in backend code, or `formatted_read_group` for a formatted result.",
        rename: None,
    },
    DeprecatedMethod {
        name: "check_field_access_rights",
        since: OdooVersion::new(19, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        replacement: "Use `check_field_access` (`_check_field_access` before 20.0), or \
                      `fields_get` to list the allowed fields.",
        rename: None,
    },
    DeprecatedMethod {
        name: "toggle_active",
        since: OdooVersion::new(19, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        reintroduced: None,
        legacy_keywords: &[],
        replacement: "Use `action_archive` or `action_unarchive` depending on the intent.",
        rename: None,
    },
];

/// ODW8502
pub(crate) fn deprecated_odoo_method_call(checker: &Checker, call: &ast::ExprCall) {
    // A bare `read_group(...)` is a plain function, not an ORM call.
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
        return;
    };
    let Some(method) = DEPRECATED_ORM_METHODS
        .iter()
        .find(|method| method.name == attr.as_str())
    else {
        return;
    };
    if !odoo_version_applies(checker, Some(method.since), method.until) {
        return;
    }
    // Say "removed" only once the project is known to be on a version without the method. With
    // no `odoo-version` configured the rule cannot tell, so it keeps the deprecation wording —
    // hence the setting is read directly rather than through `odoo_version_applies`, which
    // deliberately answers "applies" to an unconfigured project.
    let removed = method.removed.filter(|removed| {
        checker
            .settings()
            .odoo
            .odoo_version
            .is_some_and(|configured| configured >= *removed)
    });

    // Past the version that reused the name, the call is ambiguous: the same expression is
    // either a stale pre-replacement call or a correct use of the new method, and only a
    // keyword the new signature does not accept tells them apart. Report nothing otherwise —
    // a false positive here would land on code that is right.
    let replaced = method.reintroduced.filter(|reintroduced| {
        checker
            .settings()
            .odoo
            .odoo_version
            .is_some_and(|configured| configured >= *reintroduced)
    });
    let legacy_keyword = if replaced.is_some() {
        let Some(keyword) = call.arguments.keywords.iter().find_map(|keyword| {
            let name = keyword.arg.as_ref()?;
            method
                .legacy_keywords
                .contains(&name.as_str())
                .then(|| name.to_string())
        }) else {
            return;
        };
        Some(keyword)
    } else {
        None
    };

    // Only inside a method of an Odoo model class, mirroring pylint-odoo's scoping. The
    // receiver itself is left unchecked: it is routinely a recordset held in a local
    // (`orders.toggle_active()`), which no amount of static analysis would resolve.
    let semantic = checker.semantic();
    let ScopeKind::Function(function_def) = semantic.current_scope().kind else {
        return;
    };
    if !semantic.current_scopes().any(
        |scope| matches!(scope.kind, ScopeKind::Class(class_def) if is_odoo_model_class(semantic, class_def)),
    ) {
        return;
    }

    // An override of a deprecated method has to call `super().<same name>(...)` to keep the
    // chain working; Odoo's own addons do exactly that, tagged `@api.deprecated("Override of
    // a deprecated method")`. The override itself is the thing to remove, not this call.
    if function_def.name.as_str() == method.name
        && let Expr::Call(super_call) = value.as_ref()
        && matches!(super_call.func.as_ref(), Expr::Name(name) if name.id == "super")
    {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(
        DeprecatedOdooMethodCall {
            name: method.name.to_string(),
            since: method.since,
            removed,
            replaced,
            legacy_keyword,
            replacement: method.replacement.to_string(),
            rename: method.rename.map(ToString::to_string),
        },
        attr.range(),
    );
    if let Some(rename) = method.rename {
        diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
            rename.to_string(),
            attr.range(),
        )));
    }
}
