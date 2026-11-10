use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_semantic::SemanticModel;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_field_type, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for a `Char`, `Text` or `Binary` field of an Odoo model that holds a credential --
/// an API key, a secret, a password, a token -- and is not restricted to
/// `base.group_system`.
///
/// A field is taken for a credential when `password=True` is passed to it or when its name
/// ends in one of `api_key`, `api_secret`, `secret`, `secret_key`, `client_secret`,
/// `webhook_secret`, `auth_token`, `bearer_token`, `refresh_token`, `private_key`,
/// `password`, `passwd`, `passcode`, `transaction_key`, `signature_key` or
/// `signature_secret`, as a whole name or as its last words (`stripe_secret_key`). The
/// rule only applies from Odoo 15.0, the series in which the payment providers of core
/// moved their credentials from `base.group_user` to `base.group_system`.
///
/// ## Why is this bad?
/// `password=True` only masks the input in the form view. It does not stop the field from
/// being returned by `read` and `search_read`, so any user who can read the record reads the
/// secret over RPC. `res.company` and `payment.provider` are readable by portal and internal
/// users alike. `groups="base.group_system"` is what removes the field from `fields_get`,
/// `read` and `search_read` for everyone else.
///
/// `groups` takes a comma-separated list in which any group grants access, so a list that
/// names `base.group_system` next to a laxer group is reported as well.
///
/// Fields of a `TransientModel` are not reported: the settings wizard, where
/// `res.config.settings` mirrors a company field, only lives for one session and stores
/// nothing of its own. Neither is a `groups` that is not a string literal, since its value
/// can not be known.
///
/// The check goes by the name of the field and can not see the access rights of the model.
/// A model whose `ir.model.access.csv` already grants read to `base.group_system` alone does
/// not need the group on its fields, and reports a field it already protects; give that one
/// the group anyway, or silence the line.
///
/// ## Example
/// ```python
/// class ResCompany(models.Model):
///     _inherit = "res.company"
///
///     twilio_auth_token = fields.Char(password=True)
///     clover_api_key = fields.Char(groups="base.group_user")
/// ```
///
/// Use instead:
/// ```python
/// class ResCompany(models.Model):
///     _inherit = "res.company"
///
///     twilio_auth_token = fields.Char(password=True, groups="base.group_system")
///     clover_api_key = fields.Char(groups="base.group_system")
/// ```
///
/// No fix is offered. Restricting a field hides it from every user outside the group, so any
/// code that runs as one of them and reads the value has to move to `sudo()` first, and that
/// is a decision about the code that reads it.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.17.0.37", category = Category::Security)]
pub(crate) struct CredentialFieldUnrestricted {
    name: String,
    groups: Option<String>,
}

impl Violation for CredentialFieldUnrestricted {
    #[derive_message_formats]
    fn message(&self) -> String {
        let CredentialFieldUnrestricted { name, groups } = self;
        match groups {
            Some(groups) => format!(
                "Credential field `{name}` is readable by `{groups}`; restrict it to \
                 `groups=\"base.group_system\"`"
            ),
            None => format!(
                "Credential field `{name}` is readable by every user who can read the \
                 record; add `groups=\"base.group_system\"`"
            ),
        }
    }
}

/// The group that keeps a credential away from everyone but the administrators.
const SYSTEM_GROUP: &str = "base.group_system";

/// The first Odoo series whose own credential fields are restricted to [`SYSTEM_GROUP`].
const SINCE: OdooVersion = OdooVersion::new(15, 0);

/// Field types that can hold the text of a credential.
const CREDENTIAL_FIELD_TYPES: [&str; 3] = ["Char", "Text", "Binary"];

/// Names, or last words of a name, that mark a field as a credential.
const CREDENTIAL_SUFFIXES: [&str; 16] = [
    "api_key",
    "api_secret",
    "secret",
    "secret_key",
    "client_secret",
    "webhook_secret",
    "auth_token",
    "bearer_token",
    "refresh_token",
    "private_key",
    "password",
    "passwd",
    "passcode",
    "transaction_key",
    "signature_key",
    "signature_secret",
];

/// Returns `true` if `name` is one of [`CREDENTIAL_SUFFIXES`] or ends in one after an
/// underscore.
fn is_credential_name(name: &str) -> bool {
    CREDENTIAL_SUFFIXES.iter().any(|suffix| {
        name.strip_suffix(suffix)
            .is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('_'))
    })
}

/// Returns `true` if the class derives from `odoo.models.TransientModel`.
fn is_transient_model(semantic: &SemanticModel, class_def: &ast::StmtClassDef) -> bool {
    class_def.arguments.as_deref().is_some_and(|arguments| {
        arguments.args.iter().any(|base| {
            semantic
                .resolve_qualified_name(base)
                .is_some_and(|name| name.segments() == ["odoo", "models", "TransientModel"])
        })
    })
}

/// ODE9504
pub(crate) fn credential_field_unrestricted(checker: &Checker, class_def: &ast::StmtClassDef) {
    if !odoo_version_applies(checker, Some(SINCE), None) {
        return;
    }
    let semantic = checker.semantic();
    if !is_odoo_model_class(semantic, class_def) || is_transient_model(semantic, class_def) {
        return;
    }
    for stmt in &class_def.body {
        let Stmt::Assign(ast::StmtAssign { targets, value, .. }) = stmt else {
            continue;
        };
        let Expr::Call(call) = value.as_ref() else {
            continue;
        };
        if !odoo_field_type(&call.func).is_some_and(|ty| CREDENTIAL_FIELD_TYPES.contains(&ty)) {
            continue;
        }
        // A `**options` argument can carry `groups` or `password`, and there is no telling.
        if call
            .arguments
            .keywords
            .iter()
            .any(|keyword| keyword.arg.is_none())
        {
            continue;
        }
        let keyword = |name: &str| {
            call.arguments
                .find_keyword(name)
                .map(|keyword| &keyword.value)
        };
        let is_password = matches!(
            keyword("password"),
            Some(Expr::BooleanLiteral(ast::ExprBooleanLiteral {
                value: true,
                ..
            }))
        );
        let groups = match keyword("groups") {
            None => None,
            Some(Expr::StringLiteral(literal)) => Some(literal.value.to_str()),
            Some(_) => continue,
        };
        // Any group of the list grants access, so only a list made of the system group alone
        // keeps the field to the administrators.
        if groups.is_some_and(|groups| groups.split(',').all(|group| group.trim() == SYSTEM_GROUP))
        {
            continue;
        }
        for target in targets {
            let Expr::Name(name) = target else {
                continue;
            };
            if !(is_password || is_credential_name(name.id.as_str())) {
                continue;
            }
            checker.report_diagnostic(
                CredentialFieldUnrestricted {
                    name: name.id.to_string(),
                    groups: groups.map(str::to_string),
                },
                name.range(),
            );
        }
    }
}
