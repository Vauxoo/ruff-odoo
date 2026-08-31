use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::Expr;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::importer::ImportRequest;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for uses of the `odoo.osv.expression` module, deprecated in Odoo 19.0 in favor of
/// `odoo.fields.Domain`.
///
/// ## Why is this bad?
/// Every function of `odoo.osv.expression` raises a `DeprecationWarning` in 19.0, and
/// importing `odoo.osv` at all raises one too. The whole package was then removed, so code
/// still calling it fails to import on the versions that follow.
///
/// ## Example
/// ```python
/// from odoo.osv import expression
///
/// domain = expression.AND([domain, [("state", "=", "draft")]])
/// ```
///
/// Use instead:
/// ```python
/// from odoo.fields import Domain
///
/// domain = Domain.AND([domain, [("state", "=", "draft")]])
/// ```
///
/// ## Fix safety
/// The fix is marked unsafe because the two APIs do not return the same type: `expression.AND`
/// returns a plain `list`, `Domain.AND` returns a `Domain`. The ORM takes either one, and
/// `Domain` supports iteration, `+` and `==` against a list, but it has no `len()` and no
/// indexing, so a caller that measures or subscripts the result changes meaning.
///
/// The now-unused `from odoo.osv import expression` is left in place; `F401` reports it.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.33")]
pub(crate) struct DeprecatedOsvExpression {
    name: String,
    replacement: String,
    rewrite: Option<String>,
}

impl Violation for DeprecatedOsvExpression {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let DeprecatedOsvExpression {
            name, replacement, ..
        } = self;
        format!("`odoo.osv.expression.{name}` is deprecated since Odoo 19.0. {replacement}")
    }

    fn fix_title(&self) -> Option<String> {
        let rewrite = self.rewrite.as_ref()?;
        Some(format!("Replace with `{rewrite}`"))
    }
}

/// A member of `odoo.osv.expression`, and how to move off it.
struct DeprecatedSymbol {
    /// The member name, as `odoo/osv/expression.py` spells it in 19.0.
    name: &'static str,
    /// Prose naming the replacement, appended to the diagnostic message.
    replacement: &'static str,
    /// The `Domain` attribute the member maps onto one-for-one, set only where the 19.0
    /// module is a plain forwarder to it, so that swapping the reference is enough.
    attribute: Option<&'static str>,
}

/// Every public member of `odoo/osv/expression.py` in Odoo 19.0.
///
/// The module is a deprecation shim there — each function warns and delegates to
/// `odoo/orm/domains.py` — and the whole `odoo/osv` package is gone in the versions that
/// follow, so the list is closed and will not grow. The four entries carrying an `attribute`
/// are the ones whose shim is a pure delegation; the rest need the call site rewritten by
/// hand, since the replacement takes different arguments or returns a different shape.
const DEPRECATED_SYMBOLS: &[DeprecatedSymbol] = &[
    DeprecatedSymbol {
        name: "AND",
        replacement: "Use `Domain.AND` instead.",
        attribute: Some("AND"),
    },
    DeprecatedSymbol {
        name: "OR",
        replacement: "Use `Domain.OR` instead.",
        attribute: Some("OR"),
    },
    DeprecatedSymbol {
        name: "TRUE_DOMAIN",
        replacement: "Use `Domain.TRUE` instead.",
        attribute: Some("TRUE"),
    },
    DeprecatedSymbol {
        name: "FALSE_DOMAIN",
        replacement: "Use `Domain.FALSE` instead.",
        attribute: Some("FALSE"),
    },
    DeprecatedSymbol {
        name: "normalize_domain",
        // `Domain` normalizes on construction, and returns a `Domain` where the deprecated
        // function returned the normalized list.
        replacement: "Use `Domain(...)` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "distribute_not",
        replacement: "Use `Domain(...)` instead — it distributes the negation on construction.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "is_false",
        replacement: "Use `Domain(...).is_false()` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "domain_combine_anies",
        replacement: "Use `Domain(...).optimize(model)` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "combine",
        replacement: "Use `Domain.AND` or `Domain.OR` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "TRUE_LEAF",
        replacement: "Use `Domain.TRUE` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "FALSE_LEAF",
        replacement: "Use `Domain.FALSE` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "NOT_OPERATOR",
        replacement: "Use the `Domain` operators (`~`, `&`, `|`) instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "AND_OPERATOR",
        replacement: "Use the `Domain` operators (`~`, `&`, `|`) instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "OR_OPERATOR",
        replacement: "Use the `Domain` operators (`~`, `&`, `|`) instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "TERM_OPERATORS_NEGATION",
        replacement: "Use `~Domain(...)` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "NEGATIVE_TERM_OPERATORS",
        replacement: "Use `Domain.NEGATIVE_OPERATORS` instead — it is a mapping, not a set.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "DOMAIN_OPERATORS",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "TERM_OPERATORS",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "normalize_leaf",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "is_operator",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "is_leaf",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "is_boolean",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "check_leaf",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "prettify_domain",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "get_unaccent_wrapper",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
    DeprecatedSymbol {
        name: "expression",
        replacement: "Use `odoo.fields.Domain` instead.",
        attribute: None,
    },
];

/// ODW9502
pub(crate) fn deprecated_osv_expression(checker: &Checker, expr: &Expr) {
    // `odoo.fields.Domain` only exists from 19.0, which is also the version that deprecated
    // `odoo.osv.expression`.
    if !odoo_version_applies(checker, Some(OdooVersion::new(19, 0)), None) {
        return;
    }
    let semantic = checker.semantic();
    let Some(qualified_name) = semantic.resolve_qualified_name(expr) else {
        return;
    };
    // Only a member of the module, never the module itself: `expression.AND` is visited both
    // as the whole attribute and as its `expression` value, and reporting the module too
    // would flag the same call twice.
    let member = match qualified_name.segments() {
        ["odoo", "osv", "expression", member] => *member,
        _ => return,
    };
    let Some(symbol) = DEPRECATED_SYMBOLS
        .iter()
        .find(|symbol| symbol.name == member)
    else {
        return;
    };

    let mut diagnostic = checker.report_diagnostic(
        DeprecatedOsvExpression {
            name: symbol.name.to_string(),
            replacement: symbol.replacement.to_string(),
            rewrite: symbol
                .attribute
                .map(|attribute| format!("Domain.{attribute}")),
        },
        expr.range(),
    );
    if let Some(attribute) = symbol.attribute {
        diagnostic.try_set_fix(|| {
            let (import_edit, binding) = checker.importer().get_or_import_symbol(
                &ImportRequest::import_from("odoo.fields", "Domain"),
                expr.start(),
                checker.semantic(),
            )?;
            Ok(Fix::unsafe_edits(
                Edit::range_replacement(format!("{binding}.{attribute}"), expr.range()),
                [import_edit],
            ))
        });
    }
}
