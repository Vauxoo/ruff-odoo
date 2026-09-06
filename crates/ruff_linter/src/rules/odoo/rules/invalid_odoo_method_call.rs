use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::{ScopeKind, SemanticModel};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{
    class_declares_model, class_defines_method, is_odoo_model_class,
};
use crate::rules::odoo::settings::OdooVersion;
use crate::rules::odoo::signatures::{ArgumentMismatch, SHIPPED_VERSIONS, signatures_for};
use crate::warn_user_once;

/// ## What it does
/// Checks calls to Odoo ORM model methods against the parameter list the method actually
/// has in the configured [`odoo-version`](../settings.md#lint_odoo_odoo-version).
///
/// ## Why is this bad?
/// Odoo reshapes ORM signatures between releases, and the call sites do not fail until the
/// line runs. `read_group` is the extreme case: 20.0 reused the freed name for a different
/// method, so `self.read_group(domain, fields, groupby, lazy=False)` raises
/// `TypeError: got an unexpected keyword argument 'lazy'` there while reading as valid code.
/// The same happens more quietly elsewhere — `_search` dropped `access_rights_uid` in 18.0,
/// `name_search` renamed `args` to `domain` in 19.0 — and a migration has no way to find
/// them short of running every branch.
///
/// The check needs signatures to compare against, so it reports nothing unless `odoo-version`
/// is set to a version this linter ships a stub for. Since that silence is indistinguishable
/// from a clean run, it warns once on stderr when the setting is missing or names a version
/// with no signatures; the run still succeeds, and the warning only appears when this rule is
/// enabled.
///
/// Only receivers that are certainly recordsets are checked: `self` and `super()` inside a
/// model class, `env[...]` subscripts anywhere, and chains of recordset-returning ORM calls
/// over either, so `self.env["res.partner"].sudo().search(...)` is covered. A local is not,
/// because nothing distinguishes one holding a recordset from one holding a worksheet. A
/// method the file defines itself is left alone too, since the call may well mean that
/// override rather than Odoo's -- except through `env["other.model"]`, which reaches a model
/// the override says nothing about.
///
/// ## Known limitation
/// The check binds arguments; it does not know that a name changed meaning. Where Odoo
/// reused a freed name for a method of the same arity, a fully positional call still binds
/// and is not reported, even though every argument now lands on a different parameter:
///
/// ```python
/// # Silent on 20.0: seven positionals against a signature that takes seven.
/// super().read_group(domain, fields, groupby, offset, limit, orderby, lazy)
/// ```
///
/// Nothing in that call distinguishes it from correct 20.0 code, so reporting it would land
/// on code that is right. Naming a repurposed method is
/// [`deprecated-odoo-method-call`](deprecated-odoo-method-call.md)'s job, not this one's.
///
/// ## Example
/// ```python
/// groups = self.read_group(domain, ["amount:sum"], ["partner_id"], lazy=False)
/// ```
///
/// Use instead:
/// ```python
/// groups = self._read_group(domain, ["partner_id"], ["amount:sum"])
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.34")]
pub(crate) struct InvalidOdooMethodCall {
    name: String,
    version: OdooVersion,
    mismatch: ArgumentMismatch,
}

impl Violation for InvalidOdooMethodCall {
    #[derive_message_formats]
    fn message(&self) -> String {
        let InvalidOdooMethodCall {
            name,
            version,
            mismatch,
        } = self;
        match mismatch {
            ArgumentMismatch::UnexpectedKeyword(keyword) => {
                format!("`{name}` has no parameter `{keyword}` in Odoo {version}")
            }
            ArgumentMismatch::TooManyPositional { given, accepted } => format!(
                "`{name}` takes at most {accepted} argument{} in Odoo {version}, but {given} were given",
                if *accepted == 1 { "" } else { "s" }
            ),
            ArgumentMismatch::MissingRequired(parameter) => {
                format!("`{name}` requires argument `{parameter}` in Odoo {version}")
            }
            ArgumentMismatch::Duplicate(parameter) => {
                format!("`{name}` got two values for argument `{parameter}` in Odoo {version}")
            }
        }
    }
}

/// ODE9502
pub(crate) fn invalid_odoo_method_call(checker: &Checker, call: &ast::ExprCall) {
    // With no version configured there is nothing to compare against, and unlike the
    // version-scoped rules this one cannot fall back to "report anyway": which signature is
    // right *is* the question. Same for a version no stub ships for.
    //
    // Both cases turn the rule into a silent no-op, which is indistinguishable from a clean
    // run, so each says so once on stderr. They are warnings, not diagnostics: the run still
    // succeeds, and nothing is emitted unless this rule is enabled, since the dispatch site
    // is gated on that.
    let Some(version) = checker.settings().odoo.odoo_version else {
        {
            warn_user_once!(
                "ODE9502 (invalid-odoo-method-call) needs `lint.odoo.odoo-version` to know \
                 which Odoo method signatures to check calls against; skipping it."
            );
        }
        return;
    };
    let Some(signatures) = signatures_for(version) else {
        {
            warn_user_once!(
                "ODE9502 (invalid-odoo-method-call) ships no Odoo method signatures for \
                 version {version}; skipping it. Signatures are available for {}.",
                SHIPPED_VERSIONS
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return;
    };
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
        return;
    };
    let semantic = checker.semantic();
    let Some(receiver) = recordset_receiver(semantic, value) else {
        return;
    };

    // A method the file redefines may legitimately take other arguments, so a call that
    // could mean that override is left alone. Which calls those are depends on the receiver.
    let overridden_here = |class_def: &ast::StmtClassDef| match &receiver {
        // `super().<method>` names the implementation being overridden, which is exactly
        // the one this rule knows.
        Receiver::Super => false,
        Receiver::SelfRecord => class_defines_method(class_def, attr),
        // An override belongs to the model its class declares. A call routed through
        // `env["other.model"]` reaches a different model, which that override says nothing
        // about, so it stays checked. Only a call naming this class's own model -- or one
        // whose model is not a literal, where there is no telling -- is left alone.
        Receiver::Environment { model } => {
            class_defines_method(class_def, attr)
                && model
                    .as_deref()
                    .is_none_or(|model| class_declares_model(class_def, model))
        }
    };
    if semantic.current_scopes().any(
        |scope| matches!(scope.kind, ScopeKind::Class(class_def) if overridden_here(class_def)),
    ) {
        return;
    }

    let Some(signature) = signatures.get(attr.as_str()) else {
        return;
    };
    let Some(mismatch) = signature.mismatch(&call.arguments) else {
        return;
    };
    checker.report_diagnostic(
        InvalidOdooMethodCall {
            name: attr.to_string(),
            version,
            mismatch,
        },
        call.range(),
    );
}

/// The shapes of receiver this rule is willing to treat as a recordset.
enum Receiver {
    /// `self`, inside a model class.
    SelfRecord,
    /// `super()`, inside a model class.
    Super,
    /// An `env[...]` subscript, wherever it appears, carrying the model it names when that
    /// is a plain string -- `env[model_name]` gives `None`, and nothing can be concluded
    /// about which model it reaches.
    Environment { model: Option<String> },
}

/// The ORM methods that hand back a recordset of the same model, so that a chain built out
/// of them is still a recordset.
///
/// Deliberately short: it exists to reach `self.env["res.partner"].sudo().search(...)`, not
/// to trace arbitrary expressions. A method not listed here ends the chain, which is what
/// keeps `self.get_client(company).verifications.create(to=...)` — a Twilio client reached
/// through `self` — from being read as a recordset.
const RECORDSET_RETURNING: &[&str] = &[
    "browse",
    "exists",
    "filtered",
    "filtered_domain",
    "search",
    "search_fetch",
    "sorted",
    "sudo",
    "union",
    "with_company",
    "with_context",
    "with_env",
    "with_prefetch",
    "with_user",
];

/// Classifies `expr` as a receiver that is certainly a recordset, or `None`.
///
/// Anything else is left alone on purpose. A local holding a recordset is indistinguishable
/// from a local holding a worksheet, and `worksheet.write(row, col, value)` against
/// `BaseModel.write(self, vals)` is exactly the kind of false positive that would make the
/// rule unusable — measured on a real 16.0 codebase, unrestricted receivers produced 277
/// reports of which essentially none were about Odoo at all.
fn recordset_receiver(semantic: &SemanticModel, expr: &Expr) -> Option<Receiver> {
    // `env[...]` carries its own proof: the subscript yields a recordset whatever the
    // surrounding scope is, which is what lets the rule reach controllers too, where
    // `request.env["res.partner"].name_search(args=...)` lives.
    if let Expr::Subscript(ast::ExprSubscript { value, slice, .. }) = expr {
        let is_environment = match value.as_ref() {
            Expr::Name(ast::ExprName { id, .. }) => id == "env",
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr == "env",
            _ => false,
        };
        if is_environment {
            let model = match slice.as_ref() {
                Expr::StringLiteral(literal) => Some(literal.value.to_str().to_string()),
                _ => None,
            };
            return Some(Receiver::Environment { model });
        }
    }

    // A chain of recordset-returning calls keeps whatever the chain started as.
    if let Expr::Call(ast::ExprCall { func, .. }) = expr {
        if let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() {
            if RECORDSET_RETURNING.contains(&attr.as_str()) {
                // What the chain yields is a plain recordset, so an override defined in
                // this file does apply to the next call even when the chain started at
                // `super()`.
                return match recordset_receiver(semantic, value)? {
                    Receiver::Super => Some(Receiver::SelfRecord),
                    other => Some(other),
                };
            }
        }
    }

    let receiver = match expr {
        Expr::Name(ast::ExprName { id, .. }) if id == "self" => Receiver::SelfRecord,
        Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) if arguments.is_empty()
            && matches!(func.as_ref(), Expr::Name(name) if name.id == "super") =>
        {
            Receiver::Super
        }
        _ => return None,
    };
    // `self` only means a recordset inside a model; in a controller or a plain helper class
    // it means anything at all.
    semantic
        .current_scopes()
        .any(|scope| {
            matches!(scope.kind, ScopeKind::Class(class_def) if is_odoo_model_class(semantic, class_def))
        })
        .then_some(receiver)
}
