use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_semantic::{Binding, BindingKind, Imported};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::fix;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for `from odoo.exceptions import Warning`.
///
/// ## Why is this bad?
/// `odoo.exceptions.Warning` is a deprecated alias for `odoo.exceptions.UserError`,
/// and was removed in Odoo 15.0.
///
/// ## Example
/// ```python
/// from odoo.exceptions import Warning
/// ```
///
/// Use instead:
/// ```python
/// from odoo.exceptions import UserError
/// ```
///
/// ## Fix safety
/// The fix rewrites the import to `UserError` and renames every use of the imported name.
/// It is marked as unsafe because other modules may import `Warning` from this module, and
/// a name only referenced dynamically (e.g. via `getattr`) is not renamed. As an exception,
/// dropping a `Warning as UserError` alias changes no name at all and is safe. No fix is
/// offered when `UserError` is already bound to something other than
/// `odoo.exceptions.UserError`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct OdooExceptionWarning;

impl Violation for OdooExceptionWarning {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "`odoo.exceptions.Warning` is a deprecated alias to `odoo.exceptions.UserError`, use \
         `from odoo.exceptions import UserError`"
            .to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Replace with `UserError`".to_string())
    }
}

/// ODR8101
pub(crate) fn odoo_exception_warning(checker: &Checker, binding: &Binding) {
    let BindingKind::FromImport(import) = &binding.kind else {
        return;
    };
    if !matches!(
        import.qualified_name().segments(),
        ["odoo", "exceptions", "Warning"]
    ) {
        return;
    }
    let semantic = checker.semantic();
    let Some(source) = binding.source else {
        return;
    };
    let statement = semantic.statement(source);
    let Some(import_from) = statement.as_import_from_stmt() else {
        return;
    };
    let Some(alias) = import_from
        .names
        .iter()
        .find(|alias| alias.name.as_str() == "Warning")
    else {
        return;
    };

    let mut diagnostic = checker.report_diagnostic(OdooExceptionWarning, alias.range());

    // `Warning as UserError` already binds the right name; dropping the alias is enough,
    // and no reference has to change.
    if binding.name(checker.source()) == "UserError" {
        diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
            "UserError".to_string(),
            alias.range(),
        )));
        return;
    }

    if let Some(existing) = semantic.scopes[binding.scope]
        .get("UserError")
        .map(|binding_id| semantic.binding(binding_id))
    {
        // `UserError` is taken. Only when it is the real `odoo.exceptions.UserError` (and
        // this import is unaliased, so the member name is the bound name) can `Warning` be
        // dropped from the import in its favor.
        if !binding.is_alias()
            && let BindingKind::FromImport(existing_import) = &existing.kind
            && matches!(
                existing_import.qualified_name().segments(),
                ["odoo", "exceptions", "UserError"]
            )
        {
            diagnostic.try_set_fix(|| {
                let edit = fix::edits::remove_unused_imports(
                    std::iter::once("Warning"),
                    statement,
                    semantic.parent_statement(source),
                    checker.locator(),
                    checker.stylist(),
                    checker.indexer(),
                )?;
                Ok(
                    Fix::unsafe_edits(edit, rename_reference_edits(checker, binding))
                        .isolate(Checker::isolation(semantic.parent_statement_id(source))),
                )
            });
        }
        return;
    }

    // `UserError` is free: retarget the import (de-aliasing it if necessary) and rename
    // every reference.
    let edit = Edit::range_replacement("UserError".to_string(), alias.range());
    diagnostic.set_fix(Fix::unsafe_edits(
        edit,
        rename_reference_edits(checker, binding),
    ));
}

/// Rewrites every reference to the binding to `UserError`, quoting the replacement when the
/// reference is a string inside `__all__`.
fn rename_reference_edits(checker: &Checker, binding: &Binding) -> Vec<Edit> {
    let semantic = checker.semantic();
    binding
        .references()
        .map(|reference_id| {
            let reference = semantic.reference(reference_id);
            let replacement = if reference.in_dunder_all_definition() {
                let quote = checker.stylist().quote();
                format!("{quote}UserError{quote}")
            } else {
                "UserError".to_string()
            };
            Edit::range_replacement(replacement, reference.range())
        })
        .collect()
}
