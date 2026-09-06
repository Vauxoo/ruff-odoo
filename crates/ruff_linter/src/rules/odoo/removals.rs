//! The ORM model methods each Odoo version no longer has, one set per version.
//!
//! Odoo deletes model methods outright between releases -- `_where_calc` and `name_get` are
//! simply gone in 19.0, with no deprecation cycle to warn anyone -- and a call to one raises
//! `AttributeError` the first time the line runs. Which methods those are cannot be read off
//! the signature stubs alone: a method that merely *moved* is absent from the model classes
//! too, while still being callable on whatever it moved to. `_condition_to_sql` is the case
//! that matters, a `BaseModel` method in 18.0 and a `Field` method in 19.0, so
//! `field._condition_to_sql(...)` is correct code that a stub diff would report.
//!
//! `scripts/generate_odoo_model_stubs.py` does that subtraction against Odoo's own source
//! and writes one generated file per version, embedded here and parsed once, so the rule
//! reads a decided answer rather than re-deriving one.

use std::sync::LazyLock;

use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_module;
use rustc_hash::FxHashMap;

use crate::rules::odoo::settings::OdooVersion;

/// The removal set shipped for each Odoo version, one `LazyLock` each so that a run only
/// ever parses the version it is checking.
macro_rules! removals {
    ($name:ident, $path:literal) => {
        static $name: LazyLock<FxHashMap<String, OdooVersion>> =
            LazyLock::new(|| parse(include_str!($path)));
    };
}

removals!(REMOVED_16_0, "../../../resources/odoo/removed_160.py");
removals!(REMOVED_17_0, "../../../resources/odoo/removed_170.py");
removals!(REMOVED_18_0, "../../../resources/odoo/removed_180.py");
removals!(REMOVED_19_0, "../../../resources/odoo/removed_190.py");
removals!(REMOVED_20_0, "../../../resources/odoo/removed_200.py");

/// Read a generated removal set: the `REMOVED` mapping of method name to the version that
/// dropped it.
fn parse(source: &str) -> FxHashMap<String, OdooVersion> {
    parse_module(source)
        .expect("the generated Odoo removal sets must parse")
        .into_suite()
        .into_iter()
        .filter_map(|statement| match statement {
            Stmt::Assign(assign) => match *assign.value {
                Expr::Dict(dict) => Some(dict),
                _ => None,
            },
            _ => None,
        })
        .flat_map(|dict| dict.items)
        .filter_map(|item| {
            let (Some(Expr::StringLiteral(name)), Expr::StringLiteral(version)) =
                (item.key, &item.value)
            else {
                return None;
            };
            Some((
                name.value.to_str().to_string(),
                version
                    .value
                    .to_str()
                    .parse()
                    .expect("the generated Odoo removal sets must name valid versions"),
            ))
        })
        .collect()
}

/// The methods Odoo `version` no longer has, each mapped to the version that dropped it, or
/// `None` when no removal set ships for `version`.
///
/// The match is exact, for the same reason [`crate::rules::odoo::signatures::signatures_for`]
/// makes it exact: what a saas minor removed is precisely what this linter cannot know.
pub(crate) fn removals_for(
    version: OdooVersion,
) -> Option<&'static FxHashMap<String, OdooVersion>> {
    Some(match (version.major, version.minor) {
        (16, 0) => &REMOVED_16_0,
        (17, 0) => &REMOVED_17_0,
        (18, 0) => &REMOVED_18_0,
        (19, 0) => &REMOVED_19_0,
        (20, 0) => &REMOVED_20_0,
        _ => return None,
    })
}
