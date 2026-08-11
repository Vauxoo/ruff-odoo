---
name: "add-odoo-rule"
description: "Use this skill to add a new custom Odoo lint rule (ported from pylint-odoo or OCA's odoo-pre-commit-hooks) to this ruff fork's ODOO plugin. Triggers on: 'add odoo rule', 'new ODOO rule', 'port pylint-odoo check', 'port odoo-pre-commit-hooks check'."
---

# Add a custom Odoo rule to this ruff fork

Context: this fork adds a Vauxoo-specific `ODOO` rule plugin to Ruff, porting checks from
`pylint-odoo` and OCA's `odoo-pre-commit-hooks` so they run natively in Ruff with real autofix.
The rule group already exists (`crates/ruff_linter/src/rules/odoo/`, prefix `ODOO`, registered as
`Linter::Odoo` in `registry.rs`) — this skill is for adding one more rule to it, not for the
one-time plugin scaffold (that was done via `scripts/add_plugin.py odoo --url ... --prefix ODOO`;
only redo that if the `odoo` plugin directory has somehow been removed).

## Scope discipline — read this before starting

Only port **single-file, pure-Python-AST checks**. Explicitly out of scope for this plugin:
- Checks needing cross-file/whole-project aggregation (e.g. pylint-odoo's
  `consider-merging-classes-inherited`).
- Checks on non-Python files: XML views, CSV access rights, PO/gettext files. Ruff has no lint
  pipeline for these today (`ruff_linter` only walks `.py`/`.pyi`/`pyproject.toml`) — building one
  is a large, separate architectural project, not a "new rule".
- Manifest/directory-tree correlation checks (e.g. `file-not-used`,
  `weblate-component-too-long`).

Those stay covered by the existing `pylint-odoo` / `odoo-pre-commit-hooks` pre-commit hooks,
unchanged, running alongside this Ruff plugin. If a request doesn't fit "single Python file, AST
in, diagnostic (+ optionally an Edit-based fix) out", stop and flag it instead of forcing it in.

## Before writing code: find the real spec

Read the actual source of the check being ported before writing anything:
- pylint-odoo: `checkers/odoo_addons.py` (or `custom_logging.py` / `vim_comment.py`) in the
  `pylint-odoo` checkout — grep for the message code (e.g. `W8106`) to find the exact
  `visit_*`/`add_message` logic and any default config lists (e.g.
  `DFTL_METHOD_REQUIRED_SUPER`).
- odoo-pre-commit-hooks: `src/oca_pre_commit_hooks/checks_odoo_module_fixit_rules/<name>.py` — the
  `VALID`/`INVALID` test case lists at the bottom of each file are the clearest spec of intended
  behavior and good source material for the Ruff fixture file.

Port the *behavior*, not the *implementation* — pylint-odoo uses astroid inference
(`safe_infer`, `node.lookup`) that Ruff doesn't have; approximate with Ruff's semantic/binding
model (`checker.semantic()`) for same-file scope questions (e.g. "is this class an Odoo model" via
`ScopeKind::Class` + checking base class names), and simplify or skip anything that genuinely needs
cross-module inference (see Scope discipline above).

## Per-rule checklist

1. **Rule file** — `crates/ruff_linter/src/rules/odoo/rules/<rule_name>.rs`:
   - `#[derive(ViolationMetadata)] pub(crate) struct RuleName { ... }`
   - `impl Violation for RuleName` (or `AlwaysFixableViolation` if the fix is unconditional) with
     `message()`, and `fix_title()` if fixable. Set `const FIX_AVAILABILITY` to `Sometimes` when
     the fix isn't always offered (e.g. only for standalone-line comments, not inline ones).
   - The analysis function, doc-commented with `## What it does` / `## Why is this bad?` /
     `## Example` (with a "Use instead" counter-example) — these sections feed
     `cargo dev generate-all`'s doc generation, so keep them accurate; a malformed docstring won't
     necessarily fail the build but will produce a broken `docs/rules/<name>.md`.
   - For autofix: build the diagnostic with `checker.report_diagnostic(...)`, then
     `.try_set_fix(|| edit_fn().map(Fix::safe_edit))` (or `.set_fix(...)` directly when the fix
     can't fail). Reuse existing helpers before writing new ones:
     - `crate::fix::edits::remove_argument` — removes a positional or keyword call argument,
       comma-aware. Works for both `&Expr` and `&ast::Keyword` (it's generic over `T: Ranged`).
     - `crate::fix::edits::delete_stmt` — deletes a whole statement, handling trailing semicolons,
       lone-child-of-block (`pass` substitution), and full-line cleanup.
     - For dict-literal key/value pair removal (no existing generic helper) — see
       `crates/ruff_linter/src/rules/odoo/helpers.rs::remove_dict_item` for a worked
       comma-aware implementation to copy the pattern from (handles "not last item", "last item",
       and "only item — also eat a trailing comma if present" as three distinct cases).
2. **`rules/odoo/rules/mod.rs`** — add `pub(crate) use <rule_name>::*;` and `mod <rule_name>;`
   (both lists are alphabetically ordered by convention).
3. **Dispatch site** — wire the call behind `checker.is_rule_enabled(Rule::RuleName)`, in whichever
   file matches what the rule inspects:
   - `checkers/ast/analyze/expression.rs` — for `Expr::*` node checks (e.g. `Expr::Dict` for
     manifest checks). Add `odoo` to the `use crate::rules::{...}` import list (alphabetical).
   - `checkers/ast/analyze/statement.rs` — for `Stmt::*` node checks (e.g. `Stmt::Try` for
     except-pass, `Stmt::FunctionDef` for method checks, `Stmt::Assign` for field checks). Same
     import-list convention.
   - `checkers/ast/analyze/module.rs` — for whole-module checks that need to see the full `Suite`
     at once (e.g. "is this module-level `_logger` binding ever used anywhere in the file" —
     can't be answered from a single-node visitor, needs the full body via
     `ruff_python_ast::helpers::any_over_body`).
   - `checkers/tokens.rs` — for comment/token-stream checks (e.g. vim modelines). Loop over
     `comment_ranges` like the neighboring `ambiguous_unicode_character_comment` call does.
4. **`codes.rs`** — one line in the `// odoo` block:
   `(Odoo, "NNN") => rules::odoo::rules::RuleName,`. Codes are assigned sequentially
   (`ODOO001`, `ODOO002`, ...) — check the existing block for the next free number.
5. **⚠️ The gotcha that costs the most debugging time**: if the rule is dispatched from
   `checkers/tokens.rs` (or `checkers/physical_lines.rs` / `checkers/filesystem.rs`), it is **not
   enough** to wire the dispatch call — you must also add the rule to the matching arm of
   `Rule::lint_source()` in `registry.rs` (e.g. `| Rule::RuleName => LintSource::Tokens,`).
   Without this, `linter.rs`'s `context.iter_enabled_rules().any(|r| r.lint_source().is_tokens())`
   gate stays false, `check_tokens` never even runs, and the rule silently produces zero
   diagnostics — it compiles fine and the mistake is easy to miss. AST-dispatched rules (from
   `expression.rs`/`statement.rs`/`module.rs`) don't need this — they fall into the `_ =>
   LintSource::Ast` catch-all automatically.
6. **Naming convention** — rule struct names must read as "allow `${RuleName}`" (Clippy-style).
   `crates/ruff_linter/resources/test/disallowed_rule_names.txt` bans names starting with `use-`,
   `avoid-`, `prefer-`, `consider-`, etc. (checked by the `rule_naming_convention` test) — e.g. use
   `VimComment`, not `UseVimComment`, even if the original pylint-odoo message said "Use of vim
   comment" (that phrasing is fine for the `message()` string, just not the struct/code name).
7. **Registry ordering** — `Linter::Odoo` in `registry.rs`'s `Linter` enum must stay alphabetically
   positioned by its doc-comment name (`odoo`, between `NumPy-specific rules` and
   `[pandas-vet](...)`) — checked by the `linter_sorting` test. If a rebase moves things around,
   re-sort rather than appending at the end.
8. **Test fixture + case**:
   - `crates/ruff_linter/resources/test/fixtures/odoo/ODOONNN.py` (or `ODOONNN/__manifest__.py`
     for manifest-file-gated rules — the file must literally be named `__manifest__.py` since
     those rules check `checker.path().file_name()`; see `crates/ruff_linter/resources/test/fixtures/odoo/ODOO001/`
     for the pattern of nesting a rule-code directory to get a specific filename, mirroring how
     `pep8_naming`'s `N999` tests do `Path::new("N999/module/flake9/__init__.py")`).
   - One `#[test_case(Rule::RuleName, Path::new("ODOONNN.py"))]` per fixture in
     `crates/ruff_linter/src/rules/odoo/mod.rs`'s `#[cfg(test)] mod tests` block, using
     `crate::assert_diagnostics` + `LinterSettings::for_rule` (this is the current convention —
     don't copy `scripts/add_plugin.py`'s generated test scaffold verbatim, it uses stale
     `assert_messages!`/`.as_ref()` APIs that no longer exist; check a recent plugin's `mod.rs`,
     e.g. `flake8_bugbear/mod.rs`, for the live pattern).
   - Write the fixture to exercise both the positive case(s) and the near-miss negative cases from
     the original tool's own `VALID`/`INVALID` test lists (skip cases that only exercise the
     cross-file/non-Python scope this plugin deliberately excludes).
   - Run the standard test command from `AGENTS.md` scoped to the crate (or `cargo test -p
     ruff_linter --lib rules::odoo` as a fast fallback when `nextest` isn't installed), then
     **review** the generated/updated `.snap` files under `rules/odoo/snapshots/` — count the
     diagnostics and check the fix output by hand, don't just trust a green test run.

## Verification (run all of these before considering a rule done)

1. `cargo check --workspace` — not just `-p ruff_linter`; the `Linter` enum and rule registry are
   referenced from other crates.
2. The full `ruff_linter` test suite (not just the new rule's test) — a bad edit to a shared
   dispatch file can silently break unrelated rules:
   ```
   CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_LTO=off INSTA_FORCE_PASS=1 INSTA_UPDATE=always CARGO_PROFILE_DEV_DEBUG="line-tables-only" cargo nextest run -p ruff_linter
   ```
   (fallback: `cargo test -p ruff_linter --lib`). Pay attention to
   `registry::tests::rule_naming_convention` and `registry::tests::linter_sorting` specifically.
3. `cargo dev generate-all` — regenerates `ruff.schema.json` and `docs/rules/<name>.md` (the
   latter is gitignored, generated on demand — its successful generation without errors is itself
   a useful smoke test that the doc comment sections are well-formed).
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. `uv run --only-group dev --locked prek run --files <every file touched>` (or `uvx prek run
   --files ...` if this checkout has no `uv.lock` for `--locked` to resolve against — batch the
   file list in groups of ~5-10; a single `--files` call with 30+ paths has failed here with
   "File name too long").
6. Manual smoke test with the built binary, including `--fix`, on a small synthetic Odoo module —
   don't rely on unit tests alone to validate the CLI-level experience:
   ```
   cargo build --bin ruff
   target/debug/ruff check --select ODOO --preview --no-cache --fix <path>
   ```
7. Coverage (optional but useful when adding a non-trivial rule):
   `cargo llvm-cov -p ruff_linter --lib --summary-only -- rules::odoo` (install once with `cargo
   install cargo-llvm-cov --locked` + `rustup component add llvm-tools-preview`), then grep the
   `rules/odoo/` lines from the output.

## Before opening a PR

The working branch is very likely behind `astral/main` (Ruff moves fast). Run the
`sync-astral-upstream` skill first to rebase and resolve conflicts, re-verify, and only then hand
off to the `create-pr-mr` skill: push to `dev` (`Vauxoo-dev/ruff`), open the PR against `stb`
(`Vauxoo/ruff`) — never against `astral`.

## Usage Examples

### Example 1: Port a simple detection-only pylint-odoo check

**User:** Add the except-pass rule from pylint-odoo (W8138).
**Action:** The agent reads `odoo_addons.py`'s `visit_try`, writes
`rules/odoo/rules/except_pass.rs` dispatched from `statement.rs`'s `Stmt::Try` arm, adds the
`codes.rs` entry, writes a fixture covering flagged/unflagged cases, runs the full verification
list, and reports the result — no `registry.rs` `lint_source` change needed since it's AST-based.

### Example 2: Port an autofixable odoo-pre-commit-hooks check

**User:** Port the unused-logger check with autofix.
**Action:** The agent reads the LibCST rule's `VALID`/`INVALID` cases, writes a whole-module check
dispatched from `analyze/module.rs` using `any_over_body` to detect usage, wires an
`AlwaysFixableViolation` fix via `fix::edits::delete_stmt`, and verifies with both the unit test
snapshot and a manual `--fix` smoke test.
