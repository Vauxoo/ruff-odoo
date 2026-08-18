use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::{ScopeKind, SemanticModel};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for calls to the bare `_()`/`_lt()` translation functions.
///
/// ## Why is this bad?
/// Since Odoo 18.0, `self.env._()` is preferred over the bare `_()`/`_lt()` functions.
///
/// The rule only applies from Odoo 18.0 on: `self.env._` does not exist in earlier versions,
/// so on an older codebase the bare `_()` is the only correct call. Configure the targeted
/// version with the `odoo-version` setting; without it the rule stays enabled.
///
/// ## Example
/// ```python
/// def my_method(self):
///     return _("Hello")
/// ```
///
/// Use instead:
/// ```python
/// def my_method(self):
///     return self.env._("Hello")
/// ```
///
/// ## Fix safety
/// The rewrite is only offered where `self.env` exists: inside a method of an Odoo model,
/// and of an `http.Controller` from Odoo 19.0 on, the version that gave `Controller` its
/// `env` property. At module level, in a plain function, in a `@staticmethod`, in a nested
/// function or in a class that is not Odoo's, the call is reported without a fix, since
/// `self.env` would resolve to nothing there.
///
/// The same goes for what the call resolves to: only `odoo._`/`odoo._lt` are rewritten,
/// which covers an aliased import (`from odoo import _ as lt`) and leaves a `_` that came
/// from `gettext`, or a local of that name, reported but untouched.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct PreferEnvTranslation;

impl Violation for PreferEnvTranslation {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Better using self.env._".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Replace with `self.env._`".to_string())
    }
}

/// Returns `true` if `class_def` derives from `odoo.http.Controller`.
fn is_odoo_controller_class(semantic: &SemanticModel, class_def: &ast::StmtClassDef) -> bool {
    let Some(arguments) = class_def.arguments.as_deref() else {
        return false;
    };
    arguments.args.iter().any(|base| {
        matches!(
            semantic
                .resolve_qualified_name(base)
                .as_ref()
                .map(QualifiedName::segments),
            Some(["odoo", "http", "Controller"])
        )
    })
}

/// Returns `true` if `self.env` resolves inside `class_def`.
///
/// A model always has it. A controller only got it in Odoo 19.0 ("[IMP] core: use self.env
/// inside controllers"), which added the `env` property returning `request.env`; before that
/// a controller reaches the environment through `request.env` alone, so rewriting a call
/// there would raise `AttributeError`.
fn class_has_env(checker: &Checker, class_def: &ast::StmtClassDef) -> bool {
    let semantic = checker.semantic();
    is_odoo_model_class(semantic, class_def)
        || (is_odoo_controller_class(semantic, class_def)
            && odoo_version_applies(checker, Some(OdooVersion::new(19, 0)), None))
}

/// ODW8161
pub(crate) fn prefer_env_translation(checker: &Checker, call: &ast::ExprCall) {
    if !odoo_version_applies(checker, Some(OdooVersion::new(18, 0)), None) {
        return;
    }
    let Expr::Name(ast::ExprName { id, .. }) = call.func.as_ref() else {
        return;
    };
    // The function is Odoo's when it resolves to `odoo._`/`odoo._lt`, which also catches an
    // alias (`from odoo import _ as lt`). The bare names are reported even when the import
    // can not be resolved, the way pylint-odoo did, but only a resolved one is rewritten:
    // `self.env._` is no replacement for a `_` that comes from `gettext`.
    let is_odoo_translation = matches!(
        checker
            .semantic()
            .resolve_qualified_name(call.func.as_ref())
            .as_ref()
            .map(QualifiedName::segments),
        Some(["odoo", "_" | "_lt"])
    );
    if !is_odoo_translation && id != "_" && id != "_lt" {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(PreferEnvTranslation, call.func.range());

    if !is_odoo_translation {
        return;
    }
    // The fix needs a `self` that carries an `env`, so it is offered inside a method of an
    // Odoo class and nowhere else: at module level, in a plain function, in a `@staticmethod`
    // or in a class that is not Odoo's, `self.env` resolves to nothing.
    let mut scopes = checker.semantic().current_scopes();
    let in_self_method = scopes.find_map(|scope| match scope.kind {
        // A call in the class body is an attribute, not a method: stop rather than walk out
        // to an enclosing function.
        ScopeKind::Class(_) => Some(false),
        ScopeKind::Function(function_def) => Some(
            function_def
                .parameters
                .args
                .first()
                .is_some_and(|first_param| first_param.parameter.name.as_str() == "self"),
        ),
        _ => None,
    });
    if in_self_method != Some(true) {
        return;
    }
    let in_class_with_env = scopes.any(
        |scope| matches!(scope.kind, ScopeKind::Class(class_def) if class_has_env(checker, class_def)),
    );
    if !in_class_with_env {
        return;
    }
    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        "self.env._".to_string(),
        call.func.range(),
    )));
}
