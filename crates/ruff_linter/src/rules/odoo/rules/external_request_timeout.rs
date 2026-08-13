use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

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
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct ExternalRequestTimeout {
    method: String,
}

impl Violation for ExternalRequestTimeout {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ExternalRequestTimeout { method } = self;
        format!(
            "Use of external request method `{method}` without timeout. It could wait for a \
             long time"
        )
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

/// ODOO051
pub(crate) fn external_request_timeout(checker: &Checker, call: &ast::ExprCall) {
    if call.arguments.find_keyword("timeout").is_some() {
        return;
    }
    let Some(qualified_name) = checker.semantic().resolve_qualified_name(&call.func) else {
        return;
    };
    let Some(method) = EXTERNAL_REQUEST_TIMEOUT_METHODS
        .iter()
        .find(|method| qualified_name.segments() == **method)
    else {
        return;
    };
    checker.report_diagnostic(
        ExternalRequestTimeout {
            method: method.join("."),
        },
        call.range(),
    );
}
