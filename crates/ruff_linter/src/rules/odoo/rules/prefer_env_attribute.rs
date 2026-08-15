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
/// Checks for the `_cr`, `_uid` and `_context` shortcuts on a recordset or on `request`.
///
/// ## Why is this bad?
/// Odoo 19.0 deprecated all three in favor of reading them off the environment. They are
/// plain aliases — `BaseModel._cr` is literally `return self.env.cr` — so the rewrite is
/// behavior-preserving.
///
/// ## Example
/// ```python
/// self._cr.execute("SELECT 1")
/// lang = self._context.get("lang")
/// ```
///
/// Use instead:
/// ```python
/// self.env.cr.execute("SELECT 1")
/// lang = self.env.context.get("lang")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.14")]
pub(crate) struct PreferEnvAttribute {
    receiver: String,
    deprecated: String,
    replacement: String,
}

impl Violation for PreferEnvAttribute {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    #[derive_message_formats]
    fn message(&self) -> String {
        let PreferEnvAttribute {
            receiver,
            deprecated,
            replacement,
        } = self;
        format!(
            "Use \"{receiver}.env.{replacement}\" instead of \"{receiver}.{deprecated}\" (deprecated since 19.0)"
        )
    }

    fn fix_title(&self) -> Option<String> {
        let PreferEnvAttribute {
            receiver,
            replacement,
            ..
        } = self;
        Some(format!("Replace with `{receiver}.env.{replacement}`"))
    }
}

/// The deprecated shortcut and the `env` attribute it forwards to.
const ENV_SHORTCUTS: &[(&str, &str)] = &[("_cr", "cr"), ("_uid", "uid"), ("_context", "context")];

/// ODOO067
pub(crate) fn prefer_env_attribute(checker: &Checker, attribute: &ast::ExprAttribute) {
    // The three shortcuts were only deprecated in Odoo 19.0.
    if !odoo_version_applies(checker, Some(OdooVersion::new(19, 0)), None) {
        return;
    }
    let Some((_, replacement)) = ENV_SHORTCUTS
        .iter()
        .find(|(deprecated, _)| *deprecated == attribute.attr.as_str())
    else {
        return;
    };
    let value = attribute.value.as_ref();
    let Expr::Name(receiver) = value else {
        return;
    };
    if !receiver_is_odoo(checker, value, receiver.id.as_str()) {
        return;
    }
    let mut diagnostic = checker.report_diagnostic(
        PreferEnvAttribute {
            receiver: receiver.id.to_string(),
            deprecated: attribute.attr.to_string(),
            replacement: (*replacement).to_string(),
        },
        attribute.range(),
    );
    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        format!("{}.env.{replacement}", receiver.id),
        attribute.range(),
    )));
}

/// Returns `true` if `receiver` is something that carries an Odoo environment: `self` inside
/// an Odoo model class, or the `odoo.http.request` singleton, which deprecated the same three
/// shortcuts in 19.0.
fn receiver_is_odoo(checker: &Checker, receiver: &Expr, name: &str) -> bool {
    let semantic = checker.semantic();
    match name {
        "self" => semantic.current_scopes().any(
            |scope| matches!(scope.kind, ScopeKind::Class(class_def) if is_odoo_model_class(semantic, class_def)),
        ),
        // A local variable named `request` would shadow the import, so resolve it rather than
        // matching the name: only the real `odoo.http.request` counts.
        "request" => semantic
            .resolve_qualified_name(receiver)
            .is_some_and(|qualified_name| {
                matches!(qualified_name.segments(), ["odoo", "http", "request"])
            }),
        _ => false,
    }
}
