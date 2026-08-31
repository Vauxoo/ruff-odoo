use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;

/// ## What it does
/// Checks for Odoo model classes whose name does not correspond to the model they declare.
///
/// ## Why is this bad?
/// Odoo 19.0 adopted the convention that the class name is the model name in CamelCase, and
/// renamed every class in its own addons to match. A class still named after something else
/// is a leftover: `class EventMailRegistration` on a model that is now `event.mail.slot`
/// tells the reader the wrong thing, and the file it lives in is usually named after the
/// model too, so the class is the only place the stale name survives.
///
/// The expected name is derived the way Odoo's own migration script derived it — dots
/// separate words, and an underscore keeps its place while still capitalising what follows,
/// so `ir.actions.act_window` gives `IrActionsAct_Window`.
///
/// Names that differ only in capitalisation or in where the underscores fall are accepted:
/// Odoo itself writes `AccountEdiUBL` for `account.edi.ubl` and
/// `ImLivechatChannelMemberHistory` for `im_livechat.channel.member.history`. Only a genuine
/// difference in the letters is reported.
///
/// No fix is offered. Renaming a class means updating every reference to it, which can live
/// in any file of the addon, and this rule only ever sees one file.
///
/// ## Example
/// ```python
/// class Partner(models.Model):
///     _inherit = "res.partner"
/// ```
///
/// Use instead:
/// ```python
/// class ResPartner(models.Model):
///     _inherit = "res.partner"
/// ```
///
/// ## Options
/// - `lint.odoo.odoo-version`
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.33")]
pub(crate) struct ModelClassNameMismatch {
    class_name: String,
    model: String,
    expected: String,
}

impl Violation for ModelClassNameMismatch {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ModelClassNameMismatch {
            class_name,
            model,
            expected,
        } = self;
        format!("Class `{class_name}` does not match model `{model}`. Rename it to `{expected}`.")
    }
}

/// The Odoo version that adopted the convention: commit `f10bd8dd`, which ran the codemod
/// over 2565 files. It is not in 18.0, so a project on an earlier series is left alone —
/// there its own classes are named the way Odoo's were at the time.
const CONVENTION_SINCE: OdooVersion = OdooVersion::new(19, 0);

/// ODW9503
pub(crate) fn model_class_name_mismatch(checker: &Checker, class_def: &ast::StmtClassDef) {
    if !odoo_version_applies(checker, Some(CONVENTION_SINCE), None) {
        return;
    }
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }
    let Some(model) = declared_model(class_def) else {
        return;
    };
    // `base` is the registry root that every model already extends. Odoo's codemod skipped it
    // rather than renaming those classes to `Base`, and so does this rule.
    if model == "base" {
        return;
    }

    let expected = model_name_to_class_name(model);
    if same_letters(&expected, class_def.name.as_str()) {
        return;
    }

    checker.report_diagnostic(
        ModelClassNameMismatch {
            class_name: class_def.name.to_string(),
            model: model.to_string(),
            expected,
        },
        class_def.name.range(),
    );
}

/// The model a class declares: its `_name`, or the model it extends when there is no `_name`
/// and `_inherit` is a plain string.
///
/// A list-valued `_inherit` is deliberately ignored. `_inherit = ["mail.thread",
/// "account.move"]` extends several models and none of them names the class; Odoo's own
/// codemod skipped that form for the same reason.
fn declared_model(class_def: &ast::StmtClassDef) -> Option<&str> {
    match class_attribute(class_def, "_name") {
        // A `_name` that is not a plain string literal is not something this rule can
        // resolve, and falling through to `_inherit` would compare against the wrong model.
        Some(name) => string_literal(name),
        None => string_literal(class_attribute(class_def, "_inherit")?),
    }
}

/// The value expression of a class-level `<attribute> = ...` assignment.
fn class_attribute<'a>(class_def: &'a ast::StmtClassDef, attribute: &str) -> Option<&'a Expr> {
    class_def.body.iter().find_map(|stmt| {
        let ast::Stmt::Assign(assign) = stmt else {
            return None;
        };
        assign
            .targets
            .iter()
            .any(|target| matches!(target, Expr::Name(name) if name.id == attribute))
            .then(|| assign.value.as_ref())
    })
}

fn string_literal(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::StringLiteral(literal) => Some(literal.value.to_str()),
        _ => None,
    }
}

/// Odoo's own `model_name_to_class_name`, from the codemod that applied the convention
/// (`odoo/upgrade_code/18.1-01-rename-class.py`, added in `33690a98` and run in `f10bd8dd`):
///
/// ```python
/// ''.join(part[0].upper() + part[1:]
///         for part in re.split(r'\.', model.replace('_', '_.'))
///         if part)
/// ```
///
/// Turning `_` into `_.` before splitting is what keeps the underscore attached to the end of
/// its part while still capitalising the next one, so `ir.actions.act_window` becomes
/// `IrActionsAct_Window` and not `IrActionsActWindow`. Only the first letter of each part is
/// touched; the rest is copied through as written.
fn model_name_to_class_name(model: &str) -> String {
    let mut class_name = String::with_capacity(model.len());
    let mut at_word_start = true;
    for character in model.chars() {
        match character {
            '.' => at_word_start = true,
            '_' => {
                class_name.push('_');
                at_word_start = true;
            }
            _ if at_word_start => {
                class_name.extend(character.to_uppercase());
                at_word_start = false;
            }
            _ => class_name.push(character),
        }
    }
    class_name
}

/// Returns `true` if two identifiers differ only in capitalisation and in where their
/// underscores fall.
///
/// Odoo spells plenty of its own classes that way — `AccountEdiUBL` for `account.edi.ubl`,
/// `Im_LivechatChannelMemberHistory` written as `ImLivechatChannelMemberHistory` — and those
/// read as the model they belong to. Comparing letter by letter instead of byte for byte is
/// what keeps the rule to genuine leftovers: across Odoo 19.0's own addons it is the
/// difference between 55 classes reported and 262.
fn same_letters(left: &str, right: &str) -> bool {
    let letters = |name: &str| {
        name.chars()
            .filter(|character| *character != '_')
            .flat_map(char::to_lowercase)
            .collect::<Vec<_>>()
    };
    letters(left) == letters(right)
}
