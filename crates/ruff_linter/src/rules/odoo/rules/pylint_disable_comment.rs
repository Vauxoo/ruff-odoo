use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_source_file::LineRanges;
use ruff_text_size::{TextRange, TextSize};

use crate::Locator;
use crate::checkers::ast::LintContext;
use crate::registry::Rule;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for `# pylint: disable=...` comments that suppress checks Ruff now
/// covers, so they can be migrated to Ruff's `# noqa: ...` format.
///
/// A message is considered covered when it matches a Ruff rule name (most
/// pylint-odoo checks were ported under their original names, and many pylint
/// checks exist in Ruff under the same name, e.g. `too-many-branches`), a
/// pylint-odoo message code (e.g. `E8102`), or a known rename (e.g. pylint's
/// `too-complex` is Ruff's `complex-structure` / `C901`).
///
/// ## Why is this bad?
/// After migrating from pylint / pylint-odoo to Ruff, `# pylint: disable`
/// comments have no effect: Ruff only honors `# noqa` directives, so the
/// previously silenced diagnostics are reported again despite the suppression
/// comment.
///
/// ## Example
/// ```python
/// def action_confirm(env):
///     env.cr.commit()  # pylint: disable=invalid-commit
/// ```
///
/// Use instead:
/// ```python
/// def action_confirm(env):
///     env.cr.commit()  # noqa: ODOO017
/// ```
///
/// ## Fix safety
/// The fix is only offered for a trailing (inline) `disable` comment that
/// contains nothing but the pragma: an inline pylint `disable` and a `noqa`
/// both suppress findings on that single line, so the rewrite preserves
/// behavior. A standalone `# pylint: disable` comment applies to the rest of
/// the enclosing scope in pylint, and a `disable-next` pragma applies to the
/// following line; neither has a same-line `noqa` equivalent, so those are
/// reported without a fix and must be migrated by hand. Messages without a
/// Ruff equivalent are kept in a `# pylint: disable` comment next to the
/// inserted `noqa`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2")]
pub(crate) struct PylintDisableComment {
    codes: String,
}

impl Violation for PylintDisableComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let PylintDisableComment { codes } = self;
        format!("`pylint: disable` comment should be migrated to `# noqa: {codes}`")
    }

    fn fix_title(&self) -> Option<String> {
        let PylintDisableComment { codes } = self;
        Some(format!("Replace with `# noqa: {codes}`"))
    }
}

/// pylint / pylint-odoo messages whose Ruff counterpart lives under a different
/// rule name: renames from the port (e.g. `attribute-string-redundant` became
/// `field-string-redundant`), pylint checks Ruff implements under another
/// linter's rule (e.g. `too-complex` is mccabe's `complex-structure`), and
/// pylint-odoo message codes, which pylint accepts in `disable=` pragmas
/// interchangeably with names.
const MESSAGE_ALIASES: &[(&str, &str)] = &[
    ("attribute-string-redundant", "field-string-redundant"),
    (
        "consider-merging-classes-inherited",
        "duplicate-inherited-model-extension",
    ),
    ("prefer-env-translation", "direct-translation-call"),
    ("print-used", "print"),
    ("too-complex", "complex-structure"),
    ("translation-positional-used", "translation-positional"),
    ("use-vim-comment", "vim-comment"),
    // pylint-odoo message codes, in ODOO_MSGS order.
    ("C8101", "manifest-required-author"),
    ("C8102", "manifest-required-key"),
    ("C8103", "manifest-deprecated-key"),
    ("C8105", "license-allowed"),
    ("C8108", "method-compute"),
    ("C8109", "method-search"),
    ("C8110", "method-inverse"),
    ("C8111", "development-status-allowed"),
    ("C8112", "missing-readme"),
    ("C8116", "manifest-superfluous-key"),
    ("C8120", "manifest-summary-multiline"),
    ("E8101", "manifest-author-string"),
    ("E8102", "invalid-commit"),
    ("E8104", "manifest-maintainers-list"),
    ("E8130", "test-folder-imported"),
    ("E8135", "no-write-in-compute"),
    ("E8140", "no-raise-unlink"),
    ("E8146", "deprecated-name-get"),
    ("E8147", "inheritable-method-string"),
    ("E8148", "inheritable-method-lambda"),
    ("E8149", "deprecated-inselect-operator"),
    ("E8151", "translation-injection"),
    ("R8101", "odoo-exception-warning"),
    ("R8180", "duplicate-inherited-model-extension"),
    ("R8181", "invalid-email"),
    ("W8103", "translation-field"),
    ("W8105", "attribute-deprecated"),
    ("W8106", "method-required-super"),
    ("W8110", "missing-return"),
    ("W8111", "renamed-field-parameter"),
    ("W8113", "field-string-redundant"),
    ("W8114", "website-manifest-key-not-valid-uri"),
    ("W8115", "translation-contains-variable"),
    ("W8116", "print"),
    ("W8120", "translation-positional"),
    ("W8121", "context-overridden"),
    ("W8138", "except-pass"),
    ("W8150", "odoo-addons-relative-import"),
    ("W8155", "bad-builtin-groupby"),
    ("W8160", "deprecated-odoo-model-method"),
    ("W8162", "manifest-external-assets"),
    ("W8163", "no-search-all"),
    ("W8164", "super-method-mismatch"),
    ("W8165", "deprecated-self-cr"),
    ("W8202", "vim-comment"),
];

/// Resolves a pylint message name (or code) to the Ruff rule that covers it.
fn rule_for_message(message: &str) -> Option<Rule> {
    let name = MESSAGE_ALIASES
        .iter()
        .find_map(|(alias, ruff_name)| (*alias == message).then_some(*ruff_name))
        .unwrap_or(message);
    let rule = Rule::from_name(name).ok()?;
    // A removed rule's code would itself be flagged (e.g. by RUF102) if we
    // migrated a suppression to it.
    (!rule.is_removed()).then_some(rule)
}

#[derive(PartialEq, Eq)]
enum PragmaKind {
    Disable,
    DisableNext,
}

struct DisablePragma<'a> {
    /// Byte offset, within the comment text, of the `#` introducing the pragma.
    hash_start: usize,
    /// Byte offset, within the comment text, just past the last message name.
    names_end: usize,
    kind: PragmaKind,
    names: Vec<&'a str>,
}

/// Returns the length of the leading spaces/tabs in `text`.
fn leading_whitespace_len(text: &str) -> usize {
    text.len() - text.trim_start_matches([' ', '\t']).len()
}

/// Parses a pylint `disable` pragma out of a comment's text, mirroring
/// pylint's own pattern: `#`, optional whitespace, `pylint:`, a directive,
/// `=`, then message names running until `#`, `;`, or the end of the line.
fn parse_disable_pragma(text: &str) -> Option<DisablePragma<'_>> {
    let pylint_start = text.find("pylint")?;
    // The pragma must directly follow a `#` (plus optional whitespace), as
    // pylint requires; `# see pylint: disable=x` is not a pragma.
    let before = &text[..pylint_start];
    let hash_start = before.rfind('#')?;
    if !before[hash_start + 1..].trim().is_empty() {
        return None;
    }

    let mut cursor = pylint_start + "pylint".len();
    cursor += leading_whitespace_len(&text[cursor..]);
    let after_colon = text[cursor..].strip_prefix(':')?;
    cursor += ':'.len_utf8() + leading_whitespace_len(after_colon);

    let directive_len = text[cursor..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(text.len() - cursor);
    let kind = match &text[cursor..cursor + directive_len] {
        // `disable-msg` is pylint's deprecated spelling of `disable`.
        "disable" | "disable-msg" => PragmaKind::Disable,
        "disable-next" => PragmaKind::DisableNext,
        _ => return None,
    };
    cursor += directive_len;
    cursor += leading_whitespace_len(&text[cursor..]);
    if !text[cursor..].starts_with('=') {
        return None;
    }
    cursor += '='.len_utf8();

    let names_region_len = text[cursor..]
        .find(['#', ';'])
        .unwrap_or(text.len() - cursor);
    let names_region = &text[cursor..cursor + names_region_len];
    let names: Vec<&str> = names_region
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }

    Some(DisablePragma {
        hash_start,
        names_end: cursor + names_region.trim_end().len(),
        kind,
        names,
    })
}

/// ODOO047
pub(crate) fn pylint_disable_comment(
    context: &LintContext,
    locator: &Locator,
    comment_range: TextRange,
) {
    let text = locator.slice(comment_range);
    if !text.contains("pylint") {
        return;
    }
    let Some(pragma) = parse_disable_pragma(text) else {
        return;
    };

    let mut codes: Vec<String> = Vec::new();
    let mut unmapped: Vec<&str> = Vec::new();
    for name in &pragma.names {
        if let Some(rule) = rule_for_message(name) {
            let code = rule.noqa_code().to_string();
            if !codes.contains(&code) {
                codes.push(code);
            }
        } else {
            unmapped.push(name);
        }
    }
    if codes.is_empty() {
        return;
    }
    let codes = codes.join(", ");

    let (Ok(hash_start), Ok(names_end)) = (
        TextSize::try_from(pragma.hash_start),
        TextSize::try_from(pragma.names_end),
    ) else {
        return;
    };
    let pragma_range = TextRange::new(
        comment_range.start() + hash_start,
        comment_range.start() + names_end,
    );

    let Some(mut diagnostic) = context.report_diagnostic_if_enabled(
        PylintDisableComment {
            codes: codes.clone(),
        },
        pragma_range,
    ) else {
        return;
    };

    // Only an inline `disable` pragma has the same suppression scope (its own
    // line) as the `noqa` comment replacing it; see "Fix safety" above.
    if pragma.kind != PragmaKind::Disable {
        return;
    }
    let line_range = locator.full_line_range(comment_range.start());
    let before_comment = locator.slice(TextRange::new(line_range.start(), comment_range.start()));
    if before_comment.trim().is_empty() {
        return;
    }
    // Only rewrite when the pragma is the entire comment, so the replacement
    // can't clobber unrelated text (including a pre-existing `noqa`, which
    // always sits before or after the pragma within the comment).
    if !text[..pragma.hash_start].trim().is_empty() || !text[pragma.names_end..].trim().is_empty() {
        return;
    }

    let replacement = if unmapped.is_empty() {
        format!("# noqa: {codes}")
    } else {
        format!("# pylint: disable={}  # noqa: {codes}", unmapped.join(","))
    };
    diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
        replacement,
        comment_range,
    )));
}
