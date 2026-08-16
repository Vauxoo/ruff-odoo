use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_trivia::CommentRanges;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextLen, TextRange, TextSize};

use crate::Locator;
use crate::checkers::ast::LintContext;
use crate::fix::edits::delete_comment;
use crate::registry::Rule;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for `# pylint: disable=...` comments that suppress checks Ruff now
/// covers, so they can be migrated to Ruff's `# ruff: ...` suppression comments.
///
/// A message is considered covered when it matches a Ruff rule name (most
/// pylint-odoo checks were ported under their original names, and many pylint
/// checks exist in Ruff under the same name, e.g. `too-many-branches`), a
/// pylint-odoo message code (e.g. `E8102`), or a known rename — either across
/// linters, as pylint's `too-complex` becoming mccabe's `complex-structure`, or
/// within Ruff's own Pylint rules, which keep pylint's code but not always its
/// name (`C0415 import-outside-toplevel` is `PLC0415 import-outside-top-level`).
///
/// The pragma does not have to be alone in its comment. pylint reads a `#`
/// followed by anything and then `pylint:`, so a codebase part-way through the
/// migration usually looks like `# noqa: F401 pylint: disable=...`, and only the
/// directive is rewritten, leaving the rest of the comment untouched. Prose that
/// merely mentions a pragma (`# see pylint: disable=x for details`) is not one:
/// what tells them apart is that a real pragma is followed by nothing, or by
/// another comment, but never by more words.
///
/// ## Why is this bad?
/// After migrating from pylint / pylint-odoo to Ruff, `# pylint: disable`
/// comments have no effect: Ruff does not read them, so the previously silenced
/// diagnostics are reported again despite the suppression comment.
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
///     env.cr.commit()  # ruff: ignore[invalid-commit]
/// ```
///
/// The suppression names the rule rather than its code, because a name is what a
/// reader recognizes and, for every check ported here, it is the very name pylint
/// used. Note that Ruff only resolves a rule *name* in a suppression comment when
/// preview mode is on; with preview off only codes resolve. That is not a
/// restriction in practice — every `OD`/`OAPP` rule is itself a preview rule, so
/// preview is already on wherever these suppressions matter. The exception worth
/// knowing is the handful of messages that map onto stable upstream rules
/// (`print-used` to `print`, `too-complex` to `complex-structure`): those keep
/// firing with preview off, and a name-based suppression stops covering them.
///
/// ## Fix safety
/// Each pylint pragma has a Ruff suppression with the same scope, so all three
/// rewrites preserve behavior and are safe:
///
/// - an inline (trailing) `disable` pragma becomes `# ruff: ignore[...]` on that
///   same line;
/// - a `disable-next` pragma becomes an own-line `# ruff: ignore[...]`, which
///   applies to the statement below it;
/// - a standalone `disable` pragma governs the rest of the enclosing block, which
///   is what a `# ruff: disable[...]` / `# ruff: enable[...]` pair expresses. When
///   the pragma opens a `def` body it also covers the `def` header, as pylint does
///   for messages anchored there (`missing-return`, `method-required-super`, ...),
///   so the `disable` is placed above the header.
///
/// One deliberate difference: `disable-next` applies to the next *line* in pylint
/// and to the next *statement* in Ruff, so a multi-line statement ends up fully
/// covered rather than only on its first line. Widening a suppression can hide a
/// later diagnostic but never unsuppresses one.
///
/// A standalone pragma sharing its comment with other text is the one case left
/// without a fix: rebuilding it as a pair consumes the whole comment, which would
/// take that text with it.
///
/// Messages without a Ruff equivalent are kept in a `# pylint: disable` comment
/// next to the inserted suppression.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.3")]
pub(crate) struct PylintDisableComment {
    names: String,
}

impl Violation for PylintDisableComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let PylintDisableComment { names } = self;
        format!("`pylint: disable` comment should be migrated to `# ruff: ignore[{names}]`")
    }

    fn fix_title(&self) -> Option<String> {
        let PylintDisableComment { names } = self;
        Some(format!("Replace with `# ruff: ignore[{names}]`"))
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
    // pylint checks Ruff implements under a different name. Ruff reuses pylint's own
    // message code -- its `PLC0415` is pylint's `C0415` -- so pairing the two by code is
    // exact and only the human-readable name drifted. Without these, a `disable=` naming
    // one of them resolves to nothing and is silently left behind.
    ("assigning-non-slot", "non-slot-assignment"), // PLE0237
    ("bad-dunder-name", "bad-dunder-method-name"), // PLW3201
    ("bad-format-character", "bad-string-format-character"), // PLE1300
    ("chained-comparison", "boolean-chained-comparison"), // PLR1716
    ("comparison-of-constants", "comparison-of-constant"), // PLR0133
    ("consider-swap-variables", "swap-with-temporary-variable"), // PLR1712
    (
        "consider-using-augmented-assign",
        "non-augmented-assignment",
    ), // PLR6104
    ("consider-using-dict-items", "dict-index-missing-items"), // PLC0206
    ("consider-using-from-import", "manual-from-import"), // PLR0402
    ("consider-using-in", "repeated-equality-comparison"), // PLR1714
    ("consider-using-min-builtin", "if-stmt-min-max"), // PLR1730
    ("consider-using-sys-exit", "sys-exit-alias"), // PLR1722
    ("consider-using-ternary", "and-or-ternary"),  // PLR1706
    ("else-if-used", "collapsible-else-if"),       // PLR5501
    ("import-outside-toplevel", "import-outside-top-level"), // PLC0415
    ("init-is-generator", "yield-in-init"),        // PLE0100
    ("invalid-bool-returned", "invalid-bool-return-type"), // PLE0304
    ("invalid-bytes-returned", "invalid-bytes-return-type"), // PLE0308
    ("invalid-hash-returned", "invalid-hash-return-type"), // PLE0309
    ("invalid-index-returned", "invalid-index-return-type"), // PLE0305
    ("invalid-length-returned", "invalid-length-return-type"), // PLE0303
    ("invalid-str-returned", "invalid-str-return-type"), // PLE0307
    (
        "pointless-exception-statement",
        "useless-exception-statement",
    ), // PLW0133
    ("repeated-keyword", "repeated-keyword-argument"), // PLE1132
    ("self-cls-assignment", "self-or-cls-assignment"), // PLW0642
    ("single-string-used-for-slots", "single-string-slots"), // PLC0205
    ("subprocess-run-check", "subprocess-run-without-check"), // PLW1510
    (
        "too-many-try-statements",
        "too-many-statements-in-try-clause",
    ), // PLW0717
    ("typevar-double-variance", "type-bivariance"), // PLC0131
    (
        "typevar-name-incorrect-variance",
        "type-name-incorrect-variance",
    ), // PLC0105
    ("typevar-name-mismatch", "type-param-name-mismatch"), // PLC0132
    ("use-implicit-booleaness-not-len", "len-test"), // PLC1802
    ("use-maxsplit-arg", "missing-maxsplit-arg"),  // PLC0207
    ("use-sequence-for-iteration", "iteration-over-set"), // PLC0208
    ("use-set-for-membership", "literal-membership"), // PLR6201
    (
        "used-prior-global-declaration",
        "load-before-global-declaration",
    ), // PLE0118
    (
        "yield-inside-async-function",
        "yield-from-in-async-function",
    ), // PLE1700
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
    /// Byte offset, within the comment text, of the `pylint` keyword itself. It differs from
    /// `hash_start` when the pragma shares its comment with other text, as in
    /// `# noqa: F401 pylint: disable=...`.
    directive_start: usize,
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
    // pylint's own grammar is `#`, anything, then `pylint:` (see `OPTION_RGX` in its
    // `pragma_parser`), so anything between the two is allowed — most commonly a `noqa` added
    // during the migration, as in `# noqa: F401 pylint: disable=...`.
    let hash_start = text[..pylint_start].rfind('#')?;

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
        directive_start: pylint_start,
        names_end: cursor + names_region.trim_end().len(),
        kind,
        names,
    })
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

/// ODC8502
///
/// Runs once per file, after every other rule, because migrating a standalone pragma needs to
/// know which diagnostics actually fired inside the block it governs — see [`governed_region`].
/// It still runs before `noqa` directives are enforced, so an `ODC8502` diagnostic can itself
/// be suppressed with `# noqa: ODC8502`.
pub(crate) fn pylint_disable_comment(
    context: &mut LintContext,
    locator: &Locator,
    comment_ranges: &CommentRanges,
    suite: Option<&[Stmt]>,
) {
    if !context.is_rule_enabled(Rule::PylintDisableComment) {
        return;
    }

    for comment_range in comment_ranges {
        let text = locator.slice(comment_range);
        if !text.contains("pylint") {
            continue;
        }
        let Some(pragma) = parse_disable_pragma(text) else {
            continue;
        };

        // Prose that merely mentions a pragma — `# see pylint: disable=x for details` — is not
        // one. What separates the two is the words *after* the message names: a real pragma is
        // followed by nothing, or by a new comment such as a `noqa`, never by more prose.
        let after_names = text[pragma.names_end..].trim();
        if !(after_names.is_empty() || after_names.starts_with('#')) {
            continue;
        }

        let mut names: Vec<String> = Vec::new();
        let mut unmapped: Vec<&str> = Vec::new();
        for name in &pragma.names {
            if let Some(rule) = rule_for_message(name) {
                let rule_name = rule.name().to_string();
                if !names.contains(&rule_name) {
                    names.push(rule_name);
                }
            } else {
                unmapped.push(name);
            }
        }
        if names.is_empty() {
            continue;
        }
        let names = names.join(", ");

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
                names: names.clone(),
            },
            pragma_range,
        ) else {
            continue;
        };

        // Text the pragma shares its comment with has to survive the rewrite, so the
        // replacement narrows to the directive itself. When something precedes the directive
        // the replacement also lands mid-comment, and has to open a comment of its own for Ruff
        // to read it as a suppression.
        let leads_comment = text[..pragma.directive_start]
            .trim_start()
            .strip_prefix('#')
            .is_some_and(|before| before.trim().is_empty());
        let shares_comment = !leads_comment || !after_names.is_empty();
        let Ok(directive_start) = TextSize::try_from(pragma.directive_start) else {
            continue;
        };
        let replaced = if shares_comment {
            TextRange::new(
                comment_range.start() + directive_start,
                comment_range.start() + names_end,
            )
        } else {
            comment_range
        };
        // The `#` is only already there when the directive opens the comment and the narrowed
        // replacement leaves it standing; replacing the whole comment consumes it.
        let hash = if shares_comment && leads_comment {
            ""
        } else {
            "# "
        };

        // Whatever replaces the pragma keeps the messages Ruff has no rule for, so they stay
        // suppressed for whoever still runs pylint alongside.
        let kept_pragma = |suppression: String| {
            if unmapped.is_empty() {
                suppression
            } else {
                format!(
                    "{hash}pylint: disable={}  {suppression}",
                    unmapped.join(",")
                )
            }
        };
        let ignore_comment = format!("{hash}ruff: ignore[{names}]");

        let line_range = locator.full_line_range(comment_range.start());
        let before_comment =
            locator.slice(TextRange::new(line_range.start(), comment_range.start()));
        if !before_comment.trim().is_empty() {
            // An inline pragma and a trailing `ignore` both suppress their own line, so the
            // rewrite is a straight swap. `disable-next` written inline would refer to the
            // next line, which a trailing comment cannot express.
            if pragma.kind != PragmaKind::Disable {
                continue;
            }
            diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
                kept_pragma(ignore_comment),
                replaced,
            )));
            continue;
        }

        // An own-line `ignore` applies to the statement below it, which is what
        // `disable-next` means.
        if pragma.kind == PragmaKind::DisableNext {
            diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
                kept_pragma(ignore_comment),
                replaced,
            )));
            continue;
        }

        // A standalone `disable` governs the rest of the enclosing block, which is exactly
        // what a `disable`/`enable` pair expresses. Rebuilding it consumes the whole comment,
        // so a pragma sharing one is reported without a fix rather than losing that text.
        if shares_comment {
            continue;
        }
        let Some(suite) = suite else {
            // Without an AST (the file has a syntax error) the block can't be delimited.
            continue;
        };
        let region = governed_region(suite, comment_range, locator);
        let comment_line_start = locator.line_start(comment_range.start());
        let indent_before =
            |offset: TextSize| locator.slice(TextRange::new(locator.line_start(offset), offset));

        // `governed_region` pulls the start up to the compound statement's own header when the
        // pragma opens its body, because that header is where pylint anchors messages about the
        // statement itself. The `disable` has to go above that header to cover it.
        let opens_a_body = region.start() != comment_line_start;
        let indent = if opens_a_body {
            indent_before(region.start())
        } else {
            before_comment
        };

        let mut edits = if opens_a_body {
            vec![
                Edit::insertion(
                    format!("# ruff: disable[{names}]\n{indent}"),
                    region.start(),
                ),
                if unmapped.is_empty() {
                    delete_comment(comment_range, locator)
                } else {
                    Edit::range_replacement(
                        format!("# pylint: disable={}", unmapped.join(",")),
                        comment_range,
                    )
                },
            ]
        } else {
            vec![Edit::range_replacement(
                kept_pragma(format!("# ruff: disable[{names}]")),
                comment_range,
            )]
        };

        // A `disable` with no matching `enable` is itself a diagnostic
        // (`unmatched-suppression-comment`), so the pair always closes.
        let end = region.end();
        edits.push(if end == locator.line_start(end) {
            // The region ends on a line boundary (a module-level pragma runs to the end of the
            // file), so the `enable` becomes the next line.
            Edit::insertion(format!("{indent}# ruff: enable[{names}]\n"), end)
        } else {
            // The region ends at the last statement, which may be followed on that same line by
            // a trailing comment — anchor past it, or the comment would be pushed onto the
            // `enable` line.
            Edit::insertion(
                format!("\n{indent}# ruff: enable[{names}]"),
                locator.line_end(end),
            )
        });

        let Some((first, rest)) = edits.split_first() else {
            continue;
        };
        // Safe: the pair covers the same block the pylint pragma did, including code added to
        // it later.
        diagnostic.set_fix(Fix::safe_edits(first.clone(), rest.to_vec()));
    }
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
