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
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.14")]
pub(crate) struct DeprecatedOdooMethodCall {
    name: String,
    since: OdooVersion,
    /// The version that dropped the method, set only when the configured `odoo-version` is
    /// already at or past it, so that the message can say "removed" instead of "deprecated".
    removed: Option<OdooVersion>,
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
            replacement,
            ..
        } = self;
        match removed {
            Some(removed) => format!(
                "`{name}` was removed in Odoo {removed} (deprecated since {since}). {replacement}"
            ),
            None => format!("`{name}` is deprecated since Odoo {since}. {replacement}"),
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
    /// The last Odoo version the deprecation applies to. Only `read_group` needs one: Odoo
    /// brought the name back in 20.0 for the new API, so from there on the call is correct
    /// again and must not be reported.
    until: Option<OdooVersion>,
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

/// Methods marked `@api.deprecated` in Odoo's `odoo/orm/models.py`.
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
        // Not a rename: `raise_exception=False` has to become `has_access`, and the deprecated
        // method checks the model through `self.browse()` rather than the records in `self`.
        replacement: "Use `check_access` instead, or `has_access` where a boolean is needed; \
                      an override belongs in `_check_access`.",
        rename: None,
    },
    DeprecatedMethod {
        name: "check_access_rule",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        replacement: "Use `check_access` instead; an override belongs in `_check_access`.",
        rename: Some("check_access"),
    },
    DeprecatedMethod {
        name: "_filter_access_rules",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        replacement: "Use `_filtered_access` instead; an override belongs in `_check_access`.",
        rename: Some("_filtered_access"),
    },
    DeprecatedMethod {
        name: "_filter_access_rules_python",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        replacement: "Use `_filtered_access` instead; an override belongs in `_check_access`.",
        rename: Some("_filtered_access"),
    },
    DeprecatedMethod {
        name: "_check_recursion",
        since: OdooVersion::new(18, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
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
        replacement: "Use `not _has_cycle(...)` instead — the result is inverted.",
        rename: None,
    },
    DeprecatedMethod {
        name: "read_group",
        since: OdooVersion::new(19, 0),
        // Deleted alongside the others, but 20.0 reintroduced the name as the public spelling
        // of `_read_group`, so no stable release ever ships without it. Between the two, 20.0
        // is the one a project can be on, and there the call is correct — hence a plain upper
        // bound rather than a removal.
        removed: None,
        until: Some(OdooVersion::new(19, 0)),
        replacement: "Use `_read_group` in backend code, or `formatted_read_group` for a formatted result.",
        rename: None,
    },
    DeprecatedMethod {
        name: "check_field_access_rights",
        since: OdooVersion::new(19, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
        replacement: "Use `_check_field_access`, or `fields_get` to list the allowed fields.",
        rename: None,
    },
    DeprecatedMethod {
        name: "toggle_active",
        since: OdooVersion::new(19, 0),
        removed: REMOVED_AFTER_ODOO_19,
        until: None,
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
