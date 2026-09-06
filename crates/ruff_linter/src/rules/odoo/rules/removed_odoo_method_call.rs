use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{class_defines_method, inherits_non_builtin};
use crate::rules::odoo::removals::removals_for;
use crate::rules::odoo::settings::OdooVersion;
use crate::rules::odoo::signatures::SHIPPED_VERSIONS;
use crate::warn_user_once;

/// ## What it does
/// Checks for calls to ORM model methods that no longer exist in the configured
/// [`odoo-version`](../settings.md#lint_odoo_odoo-version).
///
/// ## Why is this bad?
/// Odoo deletes model methods between releases, most of them without a deprecation cycle:
/// `name_get`, `user_has_groups` and `copy_multi` went in 18.0, `_where_calc`,
/// `clear_caches` and `_apply_ir_rules` in 19.0. The call sites keep reading as valid Python
/// and raise `AttributeError` the first time the line runs, which on a portal controller or
/// a report means the branch nobody exercised during the migration.
///
/// Nothing else finds these. [`deprecated-odoo-method-call`](deprecated-odoo-method-call.md)
/// only knows the handful of methods Odoo marked `@api.deprecated`, and
/// [`invalid-odoo-method-call`](invalid-odoo-method-call.md) binds arguments against a
/// signature, so it goes quiet on exactly the methods that no longer have one.
///
/// The removal set is generated from Odoo's own source by
/// `scripts/generate_odoo_model_stubs.py`, which subtracts every name Odoo still defines
/// on some class before calling a method gone. That is what keeps two shapes out of it: a
/// method that moved, like `_condition_to_sql` leaving `BaseModel` for `Field` in 19.0, and
/// a name too ordinary to judge from the name alone, like `refresh`, which the ORM dropped
/// in 17.0 and a hardware driver's browser still answers to.
///
/// The check needs a removal set to look in, so it reports nothing unless `odoo-version` is
/// set to a version this linter ships one for. Since that silence is indistinguishable from
/// a clean run, it warns once on stderr when the setting is missing or names a version with
/// no set; the run still succeeds, and the warning only appears when this rule is enabled.
///
/// ## Scope
/// Any receiver counts, inside any class that inherits something which is not a Python
/// builtin. Unlike the argument-binding rules there is no need to prove the receiver is a
/// recordset: these names no longer exist anywhere in Odoo, so a call to one is either an
/// ORM call that breaks or a name the project defined itself, and the second is checked
/// for. That is what reaches the shape the migrations actually leave behind, a model looked
/// up into a local:
///
/// ```python
/// sale_obj = request.env["sale.order"]
/// query = sale_obj._where_calc(domain)
/// ```
///
/// A class that defines the method itself keeps its call, since the call means that
/// definition rather than Odoo's. A class inheriting only `object`, `Exception` or another
/// builtin is left alone: it is Python, not Odoo.
///
/// ## Example
/// ```python
/// class SaleOrder(models.Model):
///     _inherit = "sale.order"
///
///     def matching(self, domain):
///         return self._where_calc(domain)
/// ```
///
/// Use instead:
/// ```python
/// class SaleOrder(models.Model):
///     _inherit = "sale.order"
///
///     def matching(self, domain):
///         return self._search(domain)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.34")]
pub(crate) struct RemovedOdooMethodCall {
    name: String,
    removed_in: OdooVersion,
}

impl Violation for RemovedOdooMethodCall {
    #[derive_message_formats]
    fn message(&self) -> String {
        let RemovedOdooMethodCall { name, removed_in } = self;
        format!("`{name}` was removed from the Odoo ORM in {removed_in}")
    }
}

/// ODE9503
pub(crate) fn removed_odoo_method_call(checker: &Checker, call: &ast::ExprCall) {
    // Which methods are gone *is* the question, so with no version configured, or one no
    // removal set ships for, the rule cannot fall back to reporting anyway. Both cases turn
    // it into a silent no-op that reads like a clean run, so each says so once on stderr.
    // They are warnings, not diagnostics: the run still succeeds, and nothing is emitted
    // unless this rule is enabled, since the dispatch site is gated on that.
    let Some(version) = checker.settings().odoo.odoo_version else {
        {
            warn_user_once!(
                "ODE9503 (removed-odoo-method-call) needs `lint.odoo.odoo-version` to know \
                 which ORM methods are gone; skipping it."
            );
        }
        return;
    };
    let Some(removals) = removals_for(version) else {
        {
            warn_user_once!(
                "ODE9503 (removed-odoo-method-call) ships no removal set for version \
                 {version}; skipping it. Removal sets are available for {}.",
                SHIPPED_VERSIONS
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return;
    };
    let Expr::Attribute(ast::ExprAttribute { attr, .. }) = call.func.as_ref() else {
        return;
    };
    let Some(removed_in) = removals.get(attr.as_str()).copied() else {
        return;
    };

    let semantic = checker.semantic();
    let mut enclosing_classes = semantic
        .current_scopes()
        .filter_map(|scope| match scope.kind {
            ScopeKind::Class(class_def) => Some(class_def),
            _ => None,
        });
    // A class that still defines the method keeps whatever calls it: the name is back, and
    // the call reaches the definition rather than the ORM's absent one.
    let mut in_odoo_class = false;
    for class_def in &mut enclosing_classes {
        if class_defines_method(class_def, attr) {
            return;
        }
        in_odoo_class |= inherits_non_builtin(semantic, class_def);
    }
    if !in_odoo_class {
        return;
    }

    checker.report_diagnostic(
        RemovedOdooMethodCall {
            name: attr.to_string(),
            removed_in,
        },
        call.range(),
    );
}
