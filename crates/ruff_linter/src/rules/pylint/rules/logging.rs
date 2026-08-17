use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::analyze::logging;
use ruff_python_stdlib::logging::LoggingLevel;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::registry::Rule;
use crate::rules::odoo::rules::is_translation_underscore;
use crate::rules::pyflakes::cformat::CFormatSummary;

/// ## What it does
/// Checks for too few positional arguments for a `logging` format string.
///
/// ## Why is this bad?
/// A `TypeError` will be raised if the statement is run.
///
/// ## Example
/// ```python
/// import logging
///
/// try:
///     function()
/// except Exception as e:
///     logging.error("%s error occurred: %s", e)
///     raise
/// ```
///
/// Use instead:
/// ```python
/// import logging
///
/// try:
///     function()
/// except Exception as e:
///     logging.error("%s error occurred: %s", type(e), e)
///     raise
/// ```
///
/// ## Options
///
/// - `lint.logger-objects`
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.252")]
pub(crate) struct LoggingTooFewArgs;

impl Violation for LoggingTooFewArgs {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Not enough arguments for `logging` format string".to_string()
    }
}

/// ## What it does
/// Checks for too many positional arguments for a `logging` format string.
///
/// ## Why is this bad?
/// A `TypeError` will be raised if the statement is run.
///
/// ## Example
/// ```python
/// import logging
///
/// try:
///     function()
/// except Exception as e:
///     logging.error("Error occurred: %s", type(e), e)
///     raise
/// ```
///
/// Use instead:
/// ```python
/// import logging
///
/// try:
///     function()
/// except Exception as e:
///     logging.error("%s error occurred: %s", type(e), e)
///     raise
/// ```
///
/// ## Options
///
/// - `lint.logger-objects`
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.252")]
pub(crate) struct LoggingTooManyArgs;

impl Violation for LoggingTooManyArgs {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Too many arguments for `logging` format string".to_string()
    }
}

/// PLE1205
/// PLE1206
pub(crate) fn logging_call(checker: &Checker, call: &ast::ExprCall) {
    // If there are any starred arguments, abort.
    if call.arguments.args.iter().any(Expr::is_starred_expr) {
        return;
    }

    // If there are any starred keyword arguments, abort.
    if call
        .arguments
        .keywords
        .iter()
        .any(|keyword| keyword.arg.is_none())
    {
        return;
    }

    match call.func.as_ref() {
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => {
            if LoggingLevel::from_attribute(attr).is_none() {
                return;
            }
            if !logging::is_logger_candidate(
                &call.func,
                checker.semantic(),
                &checker.settings().logger_objects,
            ) {
                return;
            }
        }
        Expr::Name(_) => {
            let Some(qualified_name) = checker
                .semantic()
                .resolve_qualified_name(call.func.as_ref())
            else {
                return;
            };
            let ["logging", attribute] = qualified_name.segments() else {
                return;
            };
            if LoggingLevel::from_attribute(attribute).is_none() {
                return;
            }
        }
        _ => return,
    }

    // In Odoo the format string is usually wrapped in a translation call --
    // `_logger.error(_("%s not found"), name)` -- so the term to check against the supplied
    // arguments sits one level down. Only a `_()` handed nothing but the term is unwrapped:
    // when it takes values of its own it interpolates them itself, and the `translation-*`
    // checks own that call instead.
    let value = match call.arguments.find_positional(0) {
        Some(Expr::StringLiteral(ast::ExprStringLiteral { value, .. })) => value,
        Some(Expr::Call(translation))
            if is_translation_underscore(&translation.func)
                && translation.arguments.keywords.is_empty() =>
        {
            match &*translation.arguments.args {
                [Expr::StringLiteral(ast::ExprStringLiteral { value, .. })] => value,
                _ => return,
            }
        }
        _ => return,
    };

    let Ok(summary) = CFormatSummary::try_from(value.to_str()) else {
        return;
    };

    if summary.starred {
        return;
    }

    if !summary.keywords.is_empty() {
        return;
    }

    let num_message_args = call.arguments.args.len() - 1;
    let num_keywords = call.arguments.keywords.len();

    if checker.is_rule_enabled(Rule::LoggingTooManyArgs) {
        if summary.num_positional < num_message_args {
            checker.report_diagnostic(LoggingTooManyArgs, call.func.range());
        }
    }

    if checker.is_rule_enabled(Rule::LoggingTooFewArgs) {
        // A call supplying no values at all is reported too: `logging` leaves the term
        // uninterpolated, so the placeholders reach the log verbatim. pylint exempted that case
        // until it dropped the exemption in c23674554a7fac2fbb390cb67, and this follows suit.
        if num_keywords == 0 && summary.num_positional > num_message_args {
            checker.report_diagnostic(LoggingTooFewArgs, call.func.range());
        }
    }
}
