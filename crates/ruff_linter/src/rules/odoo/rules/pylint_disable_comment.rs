use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_trivia::CommentRanges;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextLen, TextRange, TextSize};

use crate::Locator;
use crate::checkers::ast::LintContext;
use crate::fix::edits::delete_comment;
use crate::noqa::NoqaMapping;
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
/// An inline (trailing) `disable` pragma is rewritten in place and the fix is
/// safe: an inline pylint `disable` and a `noqa` both suppress findings on that
/// single line, so the rewrite preserves behavior.
///
/// A standalone `# pylint: disable` comment applies to the rest of the enclosing
/// block, which no single `noqa` can express. Its fix deletes the pragma and puts
/// a `# noqa` on each line inside that block where one of the named rules
/// currently fires; a pragma that silences nothing is simply removed. That fix is
/// marked unsafe, because code added to the block later is no longer covered by
/// the suppression, and it is only offered when every rule the pragma names is
/// enabled in the run — a rule that isn't selected can't be observed firing, so
/// dropping its pragma could unsuppress it later.
///
/// Messages without a Ruff equivalent are kept in a `# pylint: disable` comment
/// next to the inserted `noqa`.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.3")]
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

/// pylint / pylint-odoo messages whose Ruff counterpart is not reachable by the
/// message name alone: pylint checks Ruff implements under another linter's rule
/// (e.g. `too-complex` is mccabe's `complex-structure`, `print-used` is
/// flake8-print's `print`), and pylint-odoo message codes, which pylint accepts
/// in `disable=` pragmas interchangeably with names.
///
/// Every other pylint-odoo check keeps its original name in this fork, so it
/// resolves through [`Rule::from_name`] without an entry here. The
/// `pylint_odoo_messages_are_all_mapped` test below keeps this list exhaustive.
const MESSAGE_ALIASES: &[(&str, &str)] = &[
    ("print-used", "print"),
    ("too-complex", "complex-structure"),
    // `prefer-env-attribute` covers `self._uid` and `self._context` on top of pylint-odoo's
    // `self._cr`, so it doesn't keep the original message name.
    ("deprecated-self-cr", "prefer-env-attribute"),
    // pylint-odoo message codes, in ODOO_MSGS order.
    ("C8101", "manifest-required-author"),
    ("C8102", "manifest-required-key"),
    ("C8103", "manifest-deprecated-key"),
    ("C8105", "license-allowed"),
    ("C8106", "manifest-version-format"),
    ("C8107", "translation-required"),
    ("C8108", "method-compute"),
    ("C8109", "method-search"),
    ("C8110", "method-inverse"),
    ("C8111", "development-status-allowed"),
    ("C8112", "missing-readme"),
    ("C8113", "no-wizard-in-models"),
    ("C8114", "category-allowed"),
    ("C8115", "missing-odoo-file"),
    ("C8116", "manifest-superfluous-key"),
    ("C8117", "category-allowed-app"),
    ("C8118", "missing-odoo-file-app"),
    ("C8119", "manifest-required-key-app"),
    ("C8120", "manifest-summary-multiline"),
    ("E8101", "manifest-author-string"),
    ("E8102", "invalid-commit"),
    ("E8103", "sql-injection"),
    ("E8104", "manifest-maintainers-list"),
    ("E8106", "external-request-timeout"),
    ("E8130", "test-folder-imported"),
    ("E8135", "no-write-in-compute"),
    ("E8140", "no-raise-unlink"),
    ("E8145", "manifest-behind-migrations"),
    ("E8146", "deprecated-name-get"),
    ("E8147", "inheritable-method-string"),
    ("E8148", "inheritable-method-lambda"),
    ("E8149", "deprecated-inselect-operator"),
    ("E8151", "translation-injection"),
    ("F8101", "resource-not-exist"),
    ("R8101", "odoo-exception-warning"),
    ("R8180", "consider-merging-classes-inherited"),
    ("R8181", "invalid-email"),
    ("W8103", "translation-field"),
    ("W8105", "attribute-deprecated"),
    ("W8106", "method-required-super"),
    ("W8107", "prohibited-method-override"),
    ("W8110", "missing-return"),
    ("W8111", "renamed-field-parameter"),
    ("W8113", "attribute-string-redundant"),
    ("W8114", "website-manifest-key-not-valid-uri"),
    ("W8115", "translation-contains-variable"),
    ("W8116", "print"),
    ("W8120", "translation-positional-used"),
    ("W8121", "context-overridden"),
    ("W8125", "manifest-data-duplicated"),
    ("W8138", "except-pass"),
    ("W8150", "odoo-addons-relative-import"),
    ("W8155", "bad-builtin-groupby"),
    ("W8160", "deprecated-odoo-model-method"),
    ("W8161", "prefer-env-translation"),
    ("W8162", "manifest-external-assets"),
    ("W8163", "no-search-all"),
    ("W8164", "super-method-mismatch"),
    ("W8165", "prefer-env-attribute"),
    ("W8202", "use-vim-comment"),
    // pylint-odoo's `custom_logging` checker re-publishes pylint's `logging-*`
    // messages as `translation-*`, rewriting the first `12` of each code to `83`
    // (`transform_msgs`), so `W1201` becomes `W8301` and so on.
    ("E8300", "translation-unsupported-format"),
    ("E8301", "translation-format-truncated"),
    ("E8305", "translation-too-many-args"),
    ("E8306", "translation-too-few-args"),
    ("W8301", "translation-not-lazy"),
    ("W8302", "translation-format-interpolation"),
    ("W8303", "translation-fstring-interpolation"),
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

/// A `noqa` line that a governed diagnostic needs, and the codes to put on it.
struct NoqaTarget {
    /// Start of the line the `noqa` comment has to be appended to.
    line_start: TextSize,
    codes: Vec<String>,
}

/// Where a diagnostic already reported for this file would need its `noqa` comment.
struct ReportedDiagnostic {
    code: String,
    /// Start of the line resolved through the `noqa` line mapping, so a diagnostic inside a
    /// multi-line string points at the line the directive actually has to sit on.
    noqa_line_start: TextSize,
}

/// The source region a standalone `disable` pragma governs, mirroring pylint's "the rest of
/// the enclosing block".
///
/// pylint scopes a standalone pragma to the block it sits in, which Ruff's per-line `noqa`
/// cannot express directly. The region below is what has to be covered by `noqa` comments to
/// preserve the suppression:
///
/// - inside a compound statement, the region runs from the pragma to the end of that
///   statement. A pragma written as the first line of a `def` body also covers the `def`
///   header, because that is where pylint anchors messages about the function itself
///   (`method-required-super`, `missing-return`, ...);
/// - at module level, the region runs from the pragma to the end of the file — which is
///   pylint's actual behavior for a pragma written above a `def`, even though it reads as if
///   it only applied to that function.
fn governed_region(suite: &[Stmt], pragma: TextRange, locator: &Locator) -> TextRange {
    let from_pragma = locator.line_start(pragma.start());
    let end_of_file = locator.contents().text_len();

    let Some(statement) = innermost_statement_containing(suite, pragma.start()) else {
        return TextRange::new(from_pragma, end_of_file);
    };
    // A pragma that no nested statement precedes is the first thing in the block, so it also
    // governs the compound statement's own header line.
    let start = if any_child_starts_before(statement, pragma.start()) {
        from_pragma
    } else {
        statement.start()
    };
    TextRange::new(start, statement.end())
}

/// The innermost statement whose range contains `offset`, if any.
///
/// Only compound statements can contain a comment, so the result is always the `def`/`if`/...
/// whose body the pragma sits in. A pragma at module level is contained by no statement.
fn innermost_statement_containing(suite: &[Stmt], offset: TextSize) -> Option<&Stmt> {
    let statement = suite
        .iter()
        .find(|statement| statement.range().contains(offset))?;
    let mut innermost = statement;
    let mut visitor = ChildStatements::default();
    visitor.visit_body_of(statement);
    for child in visitor.statements {
        if child.range().contains(offset)
            && let Some(deeper) =
                innermost_statement_containing(std::slice::from_ref(child), offset)
        {
            innermost = deeper;
        }
    }
    Some(innermost)
}

/// Returns `true` if any statement nested in `statement` starts before `offset`.
fn any_child_starts_before(statement: &Stmt, offset: TextSize) -> bool {
    let mut visitor = ChildStatements::default();
    visitor.visit_body_of(statement);
    visitor
        .statements
        .iter()
        .any(|child| child.start() < offset)
}

/// Collects the statements directly nested in a compound statement's bodies.
#[derive(Default)]
struct ChildStatements<'a> {
    statements: Vec<&'a Stmt>,
    depth: u32,
}

impl<'a> ChildStatements<'a> {
    fn visit_body_of(&mut self, statement: &'a Stmt) {
        self.depth = 0;
        visitor::walk_stmt(self, statement);
    }
}

impl<'a> Visitor<'a> for ChildStatements<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        // Only the direct children matter; deeper ones are reached by recursing in
        // `innermost_statement_containing`.
        self.statements.push(stmt);
        if self.depth == 0 {
            self.depth += 1;
            visitor::walk_stmt(self, stmt);
            self.depth -= 1;
        }
    }
}

/// ODOO047
///
/// Runs once per file, after every other rule, because migrating a standalone pragma needs to
/// know which diagnostics actually fired inside the block it governs — see [`governed_region`].
/// It still runs before `noqa` directives are enforced, so an `ODOO047` diagnostic can itself
/// be suppressed with `# noqa: ODOO047`.
pub(crate) fn pylint_disable_comment(
    context: &mut LintContext,
    locator: &Locator,
    comment_ranges: &CommentRanges,
    noqa_line_for: &NoqaMapping,
    suite: Option<&[Stmt]>,
) {
    if !context.is_rule_enabled(Rule::PylintDisableComment) {
        return;
    }
    let reported = reported_diagnostics(context, noqa_line_for);

    for comment_range in comment_ranges {
        let text = locator.slice(comment_range);
        if !text.contains("pylint") {
            continue;
        }
        let Some(pragma) = parse_disable_pragma(text) else {
            continue;
        };

        let mut rules: Vec<Rule> = Vec::new();
        let mut codes: Vec<String> = Vec::new();
        let mut unmapped: Vec<&str> = Vec::new();
        for name in &pragma.names {
            if let Some(rule) = rule_for_message(name) {
                let code = rule.noqa_code().to_string();
                if !codes.contains(&code) {
                    rules.push(rule);
                    codes.push(code);
                }
            } else {
                unmapped.push(name);
            }
        }
        if codes.is_empty() {
            continue;
        }
        let codes = codes.join(", ");

        let (Ok(hash_start), Ok(names_end)) = (
            TextSize::try_from(pragma.hash_start),
            TextSize::try_from(pragma.names_end),
        ) else {
            continue;
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
            continue;
        };

        // Only rewrite when the pragma is the entire comment, so the replacement can't
        // clobber unrelated text (including a pre-existing `noqa`, which always sits before
        // or after the pragma within the comment).
        if !text[..pragma.hash_start].trim().is_empty()
            || !text[pragma.names_end..].trim().is_empty()
        {
            continue;
        }

        let line_range = locator.full_line_range(comment_range.start());
        let before_comment =
            locator.slice(TextRange::new(line_range.start(), comment_range.start()));
        if !before_comment.trim().is_empty() {
            // An inline `disable` pragma already has the same suppression scope as the
            // `noqa` replacing it — its own line — so the rewrite is a straight swap.
            if pragma.kind != PragmaKind::Disable {
                continue;
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
            continue;
        }

        // A standalone pragma suppresses a whole block, which no single `noqa` can express.
        // Rebuild the suppression by putting a `noqa` on each line inside the block that the
        // pragma is actually silencing. This is only sound when every rule it names is
        // enabled: a rule left out of this run can't be observed firing, and dropping its
        // pragma would silently unsuppress it the next time it is selected.
        if !rules.iter().all(|rule| context.is_rule_enabled(*rule)) {
            continue;
        }
        let region = match (&pragma.kind, suite) {
            // `disable-next` applies to exactly the line below the pragma.
            (PragmaKind::DisableNext, _) => {
                let next_line_start = locator.full_line_end(comment_range.start());
                TextRange::new(next_line_start, locator.full_line_end(next_line_start))
            }
            (PragmaKind::Disable, Some(suite)) => governed_region(suite, comment_range, locator),
            // Without an AST (the file has a syntax error) the block can't be delimited.
            (PragmaKind::Disable, None) => continue,
        };

        // Messages Ruff has no rule for still need their pragma, so the comment is trimmed
        // down to them rather than deleted outright.
        let mut edits = vec![if unmapped.is_empty() {
            delete_comment(comment_range, locator)
        } else {
            Edit::range_replacement(
                format!("# pylint: disable={}", unmapped.join(",")),
                comment_range,
            )
        }];
        for target in noqa_targets(&reported, &codes_of(&rules), region, locator) {
            let line_end = locator.line_end(target.line_start);
            edits.push(Edit::insertion(
                format!("  # noqa: {}", target.codes.join(", ")),
                line_end,
            ));
        }
        let Some((first, rest)) = edits.split_first() else {
            continue;
        };
        // Unsafe: pylint suppresses the rule for the whole block, Ruff only for the lines
        // where it currently fires. Code added to the block later is no longer covered.
        diagnostic.set_fix(Fix::unsafe_edits(first.clone(), rest.to_vec()));
    }
}

fn codes_of(rules: &[Rule]) -> Vec<String> {
    rules
        .iter()
        .map(|rule| rule.noqa_code().to_string())
        .collect()
}

/// Snapshots the diagnostics reported so far, so the pragma rewrite can tell which lines
/// inside a governed block actually need a `noqa`.
fn reported_diagnostics(
    context: &mut LintContext,
    noqa_line_for: &NoqaMapping,
) -> Vec<ReportedDiagnostic> {
    context
        .iter()
        .filter_map(|diagnostic| {
            let code = diagnostic.secondary_code()?.to_string();
            let start = diagnostic.range()?.start();
            Some(ReportedDiagnostic {
                code,
                noqa_line_start: noqa_line_for.resolve(start),
            })
        })
        .collect()
}

/// Groups the diagnostics governed by a pragma into the `noqa` comments that would replace it.
fn noqa_targets(
    reported: &[ReportedDiagnostic],
    codes: &[String],
    region: TextRange,
    locator: &Locator,
) -> Vec<NoqaTarget> {
    let mut targets: Vec<NoqaTarget> = Vec::new();
    for diagnostic in reported {
        if !region.contains(diagnostic.noqa_line_start) || !codes.contains(&diagnostic.code) {
            continue;
        }
        let line_start = locator.line_start(diagnostic.noqa_line_start);
        // A line that already carries a `noqa` needs no second one, and appending to it would
        // produce two directives on one line.
        if locator
            .slice(TextRange::new(line_start, locator.line_end(line_start)))
            .contains("noqa")
        {
            continue;
        }
        match targets
            .iter_mut()
            .find(|target| target.line_start == line_start)
        {
            Some(target) => {
                if !target.codes.contains(&diagnostic.code) {
                    target.codes.push(diagnostic.code.clone());
                }
            }
            None => targets.push(NoqaTarget {
                line_start,
                codes: vec![diagnostic.code.clone()],
            }),
        }
    }
    targets.sort_by_key(|target| target.line_start);
    for target in &mut targets {
        target.codes.sort();
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::{MESSAGE_ALIASES, rule_for_message};
    use crate::registry::Rule;

    /// Every message pylint-odoo can emit, as `(code, name)`, taken from `ODOO_MSGS` in
    /// `pylint_odoo/checkers/odoo_addons.py`, `vim_comment.py`, and the `translation-*`
    /// messages `custom_logging.py` derives from pylint's own `logging-*` checks.
    ///
    /// Keeping this list complete is the point of the test below: a `# pylint: disable` for
    /// any of these has to resolve to the Ruff rule that replaced it, or the migration
    /// silently drops the suppression.
    const PYLINT_ODOO_MESSAGES: &[(&str, &str)] = &[
        ("C8101", "manifest-required-author"),
        ("C8102", "manifest-required-key"),
        ("C8103", "manifest-deprecated-key"),
        ("C8105", "license-allowed"),
        ("C8106", "manifest-version-format"),
        ("C8107", "translation-required"),
        ("C8108", "method-compute"),
        ("C8109", "method-search"),
        ("C8110", "method-inverse"),
        ("C8111", "development-status-allowed"),
        ("C8112", "missing-readme"),
        ("C8113", "no-wizard-in-models"),
        ("C8114", "category-allowed"),
        ("C8115", "missing-odoo-file"),
        ("C8116", "manifest-superfluous-key"),
        ("C8117", "category-allowed-app"),
        ("C8118", "missing-odoo-file-app"),
        ("C8119", "manifest-required-key-app"),
        ("C8120", "manifest-summary-multiline"),
        ("E8101", "manifest-author-string"),
        ("E8102", "invalid-commit"),
        ("E8103", "sql-injection"),
        ("E8104", "manifest-maintainers-list"),
        ("E8106", "external-request-timeout"),
        ("E8130", "test-folder-imported"),
        ("E8135", "no-write-in-compute"),
        ("E8140", "no-raise-unlink"),
        ("E8145", "manifest-behind-migrations"),
        ("E8146", "deprecated-name-get"),
        ("E8147", "inheritable-method-string"),
        ("E8148", "inheritable-method-lambda"),
        ("E8149", "deprecated-inselect-operator"),
        ("E8151", "translation-injection"),
        ("E8300", "translation-unsupported-format"),
        ("E8301", "translation-format-truncated"),
        ("E8305", "translation-too-many-args"),
        ("E8306", "translation-too-few-args"),
        ("F8101", "resource-not-exist"),
        ("R8101", "odoo-exception-warning"),
        ("R8180", "consider-merging-classes-inherited"),
        ("R8181", "invalid-email"),
        ("W8103", "translation-field"),
        ("W8105", "attribute-deprecated"),
        ("W8106", "method-required-super"),
        ("W8107", "prohibited-method-override"),
        ("W8110", "missing-return"),
        ("W8111", "renamed-field-parameter"),
        ("W8113", "attribute-string-redundant"),
        ("W8114", "website-manifest-key-not-valid-uri"),
        ("W8115", "translation-contains-variable"),
        ("W8116", "print-used"),
        ("W8120", "translation-positional-used"),
        ("W8121", "context-overridden"),
        ("W8125", "manifest-data-duplicated"),
        ("W8138", "except-pass"),
        ("W8150", "odoo-addons-relative-import"),
        ("W8155", "bad-builtin-groupby"),
        ("W8160", "deprecated-odoo-model-method"),
        ("W8161", "prefer-env-translation"),
        ("W8162", "manifest-external-assets"),
        ("W8163", "no-search-all"),
        ("W8164", "super-method-mismatch"),
        ("W8165", "deprecated-self-cr"),
        ("W8202", "use-vim-comment"),
        ("W8301", "translation-not-lazy"),
        ("W8302", "translation-format-interpolation"),
        ("W8303", "translation-fstring-interpolation"),
    ];

    /// Both spellings pylint accepts in a `disable=` pragma — the message code and the
    /// message name — must resolve to the same Ruff rule.
    #[test]
    fn pylint_odoo_messages_are_all_mapped() {
        for (code, name) in PYLINT_ODOO_MESSAGES {
            let by_code = rule_for_message(code);
            assert!(
                by_code.is_some(),
                "pylint-odoo code `{code}` ({name}) does not resolve to a Ruff rule; \
                 add it to MESSAGE_ALIASES"
            );
            let by_name = rule_for_message(name);
            assert!(
                by_name.is_some(),
                "pylint-odoo message `{name}` ({code}) does not resolve to a Ruff rule; \
                 add it to MESSAGE_ALIASES"
            );
            assert_eq!(
                by_code, by_name,
                "pylint-odoo `{code}` and `{name}` resolve to different Ruff rules"
            );
        }
    }

    /// A rename on the Ruff side must not leave an alias pointing at a name that no longer
    /// exists — `rule_for_message` would silently treat it as unmapped.
    #[test]
    fn message_alias_targets_exist() {
        for (alias, ruff_name) in MESSAGE_ALIASES {
            assert!(
                Rule::from_name(ruff_name).is_ok(),
                "MESSAGE_ALIASES maps `{alias}` to `{ruff_name}`, which is not a Ruff rule"
            );
        }
    }
}
