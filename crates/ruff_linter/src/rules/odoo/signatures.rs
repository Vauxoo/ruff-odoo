//! The parameter lists of Odoo's ORM model methods, one set per Odoo version.
//!
//! Odoo reshapes these signatures between releases — `read_group` loses `lazy`, gains
//! `aggregates` and changes what its second positional means between 18.0 and 20.0 — so a
//! call that is correct on one version raises `TypeError` on the next. Transcribing that by
//! hand does not scale past a handful of methods, so the signatures are read out of Odoo's
//! own source by `scripts/generate_odoo_model_stubs.py` into one signature-only Python file
//! per version, embedded here and parsed once.
//!
//! Keeping the stubs as Python rather than as a Rust table is what makes `*`, `*args`,
//! `**kwargs` and positional-only `/` free: Ruff's own parser answers those questions, and
//! the generated files diff readably from one Odoo release to the next.

use std::sync::LazyLock;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_parser::parse_module;
use rustc_hash::FxHashMap;

use crate::rules::odoo::settings::OdooVersion;

/// The stub shipped for each Odoo version the rule knows about, one `LazyLock` each so that
/// a run only ever parses the version it is checking.
///
/// A version with no stub is not checked at all: with no signatures to bind against, every
/// answer the rule could give would be a guess.
macro_rules! stub {
    ($name:ident, $path:literal) => {
        static $name: LazyLock<FxHashMap<String, Signature>> =
            LazyLock::new(|| parse(include_str!($path)));
    };
}

stub!(MODELS_16_0, "../../../resources/odoo/models_160.py");
stub!(MODELS_17_0, "../../../resources/odoo/models_170.py");
stub!(MODELS_18_0, "../../../resources/odoo/models_180.py");
stub!(MODELS_19_0, "../../../resources/odoo/models_190.py");
stub!(MODELS_20_0, "../../../resources/odoo/models_200.py");

/// A single method's parameter list, reduced to what deciding a call's validity needs.
#[derive(Debug)]
pub(crate) struct Signature {
    /// The positional-or-keyword parameters, in order, `self` already dropped.
    positional: Vec<String>,
    /// How many of `positional` are positional-only, and so cannot be passed by keyword.
    positional_only: usize,
    /// How many of `positional` have no default and must therefore be bound.
    required: usize,
    /// The keyword-only parameters, each with whether it has to be bound.
    keyword_only: Vec<(String, bool)>,
    /// Whether the signature ends the positional count with a `*args`.
    variadic: bool,
    /// Whether the signature accepts arbitrary keywords through a `**kwargs`.
    keyword_variadic: bool,
}

/// Why a call cannot bind to the signature of the method it names.
#[derive(Debug)]
pub(crate) enum ArgumentMismatch {
    /// A keyword the signature has no parameter for, and no `**kwargs` to absorb it.
    UnexpectedKeyword(String),
    /// More positional arguments than there are parameters to take them.
    TooManyPositional { given: usize, accepted: usize },
    /// A parameter with no default that the call binds neither by position nor by keyword.
    MissingRequired(String),
    /// A parameter bound twice, once by position and once by keyword.
    Duplicate(String),
}

impl Signature {
    /// Read a signature off a stub's `def`, dropping the leading `self`.
    fn from_parameters(parameters: &ast::Parameters) -> Self {
        let mut positional: Vec<String> = parameters
            .posonlyargs
            .iter()
            .chain(&parameters.args)
            .map(|parameter| parameter.parameter.name.to_string())
            .collect();
        let mut positional_only = parameters.posonlyargs.len();
        // Every method in the stub is an instance method, and the receiver is spelled by the
        // call site rather than passed, so `self` is not a parameter as far as binding goes.
        if !positional.is_empty() {
            positional.remove(0);
            positional_only = positional_only.saturating_sub(1);
        }
        let defaults = parameters
            .posonlyargs
            .iter()
            .chain(&parameters.args)
            .skip(1)
            .filter(|parameter| parameter.default.is_some())
            .count();
        Self {
            required: positional.len() - defaults,
            positional,
            positional_only,
            keyword_only: parameters
                .kwonlyargs
                .iter()
                .map(|parameter| {
                    (
                        parameter.parameter.name.to_string(),
                        parameter.default.is_none(),
                    )
                })
                .collect(),
            variadic: parameters.vararg.is_some(),
            keyword_variadic: parameters.kwarg.is_some(),
        }
    }

    /// The first reason `arguments` cannot bind to this signature, if there is one.
    ///
    /// A call that unpacks — `*args` or `**kwargs` at the call site — hides how many
    /// arguments it really passes and which names it really binds, so each check that the
    /// unpacking could invalidate is skipped rather than guessed at.
    pub(crate) fn mismatch(&self, arguments: &ast::Arguments) -> Option<ArgumentMismatch> {
        let unpacks_positional = arguments.args.iter().any(Expr::is_starred_expr);
        let unpacks_keywords = arguments
            .keywords
            .iter()
            .any(|keyword| keyword.arg.is_none());
        let given_positional = arguments.args.len();
        let given_keywords: Vec<&str> = arguments
            .keywords
            .iter()
            .filter_map(|keyword| Some(keyword.arg.as_ref()?.as_str()))
            .collect();

        if !unpacks_keywords {
            if let Some(unexpected) = given_keywords.iter().find(|name| !self.accepts(name)) {
                return Some(ArgumentMismatch::UnexpectedKeyword(
                    (*unexpected).to_string(),
                ));
            }
        }

        if !unpacks_positional && !self.variadic && given_positional > self.positional.len() {
            return Some(ArgumentMismatch::TooManyPositional {
                given: given_positional,
                accepted: self.positional.len(),
            });
        }

        if unpacks_positional || unpacks_keywords {
            return None;
        }

        let bound_positionally = &self.positional[..given_positional.min(self.positional.len())];
        if let Some(duplicate) = bound_positionally
            .iter()
            .find(|name| given_keywords.contains(&name.as_str()))
        {
            return Some(ArgumentMismatch::Duplicate(duplicate.clone()));
        }

        let missing = self.positional[..self.required]
            .iter()
            .skip(given_positional)
            .find(|name| !given_keywords.contains(&name.as_str()))
            .or_else(|| {
                self.keyword_only
                    .iter()
                    .filter(|(_, required)| *required)
                    .map(|(name, _)| name)
                    .find(|name| !given_keywords.contains(&name.as_str()))
            });
        missing.map(|name| ArgumentMismatch::MissingRequired(name.clone()))
    }

    /// Whether a keyword named `name` binds to some parameter of this signature.
    fn accepts(&self, name: &str) -> bool {
        self.keyword_variadic
            || self.positional[self.positional_only..]
                .iter()
                .any(|parameter| parameter == name)
            || self
                .keyword_only
                .iter()
                .any(|(parameter, _)| parameter == name)
    }
}

/// Read every method of a stub into a signature, keyed by name.
fn parse(source: &str) -> FxHashMap<String, Signature> {
    parse_module(source)
        .expect("the generated Odoo model stubs must parse")
        .into_suite()
        .into_iter()
        .filter_map(|statement| match statement {
            Stmt::ClassDef(class) => Some(class.body),
            _ => None,
        })
        .flatten()
        .filter_map(|statement| match statement {
            Stmt::FunctionDef(function) => Some((
                function.name.to_string(),
                Signature::from_parameters(&function.parameters),
            )),
            _ => None,
        })
        .collect()
}

/// Every Odoo version a stub ships for, in order, for the message that says a configured
/// version is not one of them.
pub(crate) const SHIPPED_VERSIONS: &[OdooVersion] = &[
    OdooVersion::new(16, 0),
    OdooVersion::new(17, 0),
    OdooVersion::new(18, 0),
    OdooVersion::new(19, 0),
    OdooVersion::new(20, 0),
];

/// The ORM model method signatures for `version`, or `None` when no stub ships for it.
///
/// The match is exact. A `saas~18.2` deployment is not rounded down to the 18.0 stub: the
/// signatures a saas minor carries are precisely the ones this rule cannot know, and
/// answering from the nearest stable would be a guess dressed as an answer.
pub(crate) fn signatures_for(
    version: OdooVersion,
) -> Option<&'static FxHashMap<String, Signature>> {
    Some(match (version.major, version.minor) {
        (16, 0) => &MODELS_16_0,
        (17, 0) => &MODELS_17_0,
        (18, 0) => &MODELS_18_0,
        (19, 0) => &MODELS_19_0,
        (20, 0) => &MODELS_20_0,
        _ => return None,
    })
}
