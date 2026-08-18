use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::fix::edits::add_argument;
use crate::{Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for calls to external request methods (`requests.get`, `urllib.request.urlopen`,
/// `smtplib.SMTP`, ...) without an explicit `timeout` keyword argument.
///
/// ## Why is this bad?
/// Without a timeout these calls can block forever on an unresponsive peer, hanging the
/// Odoo worker that runs them.
///
/// ## Example
/// ```python
/// response = requests.get(url)
/// ```
///
/// Use instead:
/// ```python
/// response = requests.get(url, timeout=10)
/// ```
///
/// ## Overlap with S113
/// `S113` (`request-without-timeout`) detects the `requests.*`/`httpx.*` subset of this
/// rule. This rule additionally covers `ftplib`, `http.client`, `smtplib`, `serial`,
/// `suds`, `urllib.request.urlopen` and Odoo's IAP `jsonrpc`, matching pylint-odoo's
/// `external-request-timeout` default list — enable one of the two, not both, to avoid
/// duplicated reports on `requests` calls.
///
/// ## Fix safety
/// The fix inserts `timeout=120` — the number of seconds is set by
/// [`external-request-timeout-seconds`](../settings.md#lint_odoo_external-request-timeout-seconds).
/// It is marked as unsafe because a call that used to block indefinitely now raises a
/// timeout error once the limit passes. No fix is offered when the call unpacks `**kwargs`,
/// which may already carry a `timeout`.
///
/// ## Options
/// - `lint.odoo.external-request-timeout-methods`
/// - `lint.odoo.external-request-timeout-seconds`
///
/// The methods are written as dotted paths, e.g. `requests.get`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.5")]
pub(crate) struct ExternalRequestTimeout {
    method: String,
    seconds: u32,
}

impl Violation for ExternalRequestTimeout {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let ExternalRequestTimeout { method, .. } = self;
        format!(
            "Use of external request method `{method}` without timeout. It could wait for a \
             long time"
        )
    }

    fn fix_title(&self) -> Option<String> {
        let ExternalRequestTimeout { seconds, .. } = self;
        Some(format!("Add `timeout={seconds}`"))
    }
}

/// Same default list as pylint-odoo's `external-request-timeout-methods` option.
const EXTERNAL_REQUEST_TIMEOUT_METHODS: &[&[&str]] = &[
    &["ftplib", "FTP"],
    &["http", "client", "HTTPConnection"],
    &["http", "client", "HTTPSConnection"],
    &["odoo", "addons", "iap", "models", "iap", "jsonrpc"],
    &["requests", "delete"],
    &["requests", "get"],
    &["requests", "head"],
    &["requests", "options"],
    &["requests", "patch"],
    &["requests", "post"],
    &["requests", "put"],
    &["requests", "request"],
    &["serial", "Serial"],
    &["smtplib", "SMTP"],
    &["suds", "client", "Client"],
    &["urllib", "request", "urlopen"],
];

/// ODE8106
pub(crate) fn external_request_timeout(checker: &Checker, call: &ast::ExprCall) {
    if call.arguments.find_keyword("timeout").is_some() {
        return;
    }
    let Some(qualified_name) = checker.semantic().resolve_qualified_name(&call.func) else {
        return;
    };
    let Some(method) = checker
        .settings()
        .odoo
        .external_request_timeout_methods
        .matching_path(qualified_name.segments(), EXTERNAL_REQUEST_TIMEOUT_METHODS)
    else {
        return;
    };
    let seconds = checker.settings().odoo.external_request_timeout_seconds.0;
    let mut diagnostic =
        checker.report_diagnostic(ExternalRequestTimeout { method, seconds }, call.range());
    // `**kwargs` may already carry a `timeout`; adding an explicit one on top would raise a
    // `TypeError` at the call.
    if call.arguments.keywords.iter().all(|kw| kw.arg.is_some()) {
        diagnostic.set_fix(Fix::unsafe_edit(add_argument(
            &format!("timeout={seconds}"),
            &call.arguments,
            checker.tokens(),
        )));
    }
}
