---
name: "sync-astral-upstream"
description: "Use this skill to rebase this Vauxoo ruff fork onto the latest astral-sh/ruff upstream, fixing whatever merge conflicts come up. Triggers on: 'sync with astral', 'rebase from astral', 'update from upstream', 'sync upstream ruff'."
---

# Sync from astral-sh/ruff upstream

This repo is a fork chain: `astral-sh/ruff` (remote `astral`) → `Vauxoo/ruff` (remote `stb`) →
`Vauxoo-dev/ruff` (remote `dev`). Custom Odoo-specific lint rules live on top of this chain (see
the `add-odoo-rule` skill) as a new `ODOO` rule plugin under `crates/ruff_linter/src/rules/odoo/`.

Because `astral-sh/ruff` moves fast (near-weekly releases, pre-1.0 so even patches can break
things), the Odoo work branch drifts behind quickly. This skill brings it back up to date.

## Hard rule: never push to `astral`, never PR to `astral`

This skill exists purely to pull changes **from** `astral-sh/ruff` **into** this fork. It must
never:
- push any branch to the `astral` remote,
- open a pull request against `astral-sh/ruff`.

The Odoo rules are Vauxoo-internal and out of scope for upstream Ruff. If the sync surfaces a
change that looks like a genuine, generically-useful upstream-worthy fix unrelated to the Odoo
work, mention it to the user — don't act on it here.

## Workflow

1. `git fetch astral --quiet` (and `git fetch stb --quiet` for comparison — they should normally
   be identical or `stb` slightly behind `astral` if Vauxoo hasn't synced its own fork yet; if
   `stb/main` is behind `astral/main`, flag that to the user before proceeding, since the PR target
   (`stb`) should also be updated by the user/a separate process — this skill only updates the
   local working branch, it does not push to `stb`).
2. Check `git status` first — never run a rebase with uncommitted changes that aren't accounted
   for. If there are uncommitted changes:
   - If they're finished work ready to be preserved across the rebase, commit them first.
   - If they're mid-flight and the branch has zero commits ahead of its old base yet (check with
     `git rev-list --count <upstream-base>..HEAD`), it's simpler and safer to `git stash -u`,
     fast-forward the branch (`git merge --ff-only astral/main`, guaranteed conflict-free since a
     pure ancestor fast-forward doesn't touch file content), then `git stash pop` and resolve any
     conflicts that pop produces — smaller conflict surface than a full multi-commit rebase.
3. If the branch already has real commits ahead of its base, use `git rebase astral/main` instead
   (not `merge` — the user wants linear history preserved for eventual review/PR).
4. **Known conflict hotspots** for the Odoo plugin specifically — these are shared/central files
   that upstream touches constantly, so expect conflicts here on every sync:
   - `crates/ruff_linter/src/codes.rs` — the giant `code_to_rule` match. Our addition is a small
     `// odoo` block with `(Odoo, "NNN") => rules::odoo::rules::RuleName,` lines. Keep our block
     intact; take upstream's changes everywhere else. If upstream inserted/removed lines near our
     block, just re-anchor it (it doesn't need to be in any particular position, unlike the
     `Linter` enum which is alphabetically sorted).
   - `crates/ruff_linter/src/registry.rs` — the `Linter` enum (keep the `Odoo` variant
     **alphabetically positioned** — after `Numpy`, before `PandasVet` — the `linter_sorting` test
     enforces this) and the `Rule::lint_source()` match (keep the odoo `Tokens`-classified rule,
     e.g. `VimComment`, in the `LintSource::Tokens` arm — everything else defaults to
     `LintSource::Ast` via the catch-all, so only token/physical-line/filesystem rules need an
     entry here).
   - `crates/ruff_linter/src/checkers/tokens.rs` and
     `crates/ruff_linter/src/checkers/ast/analyze/{expression,statement,module}.rs` — each has one
     `if checker.is_rule_enabled(Rule::X) { odoo::rules::x(...); }` block per odoo rule, plus
     `odoo` added to the `use crate::rules::{...}` import list. Keep our blocks; they're additive
     and independent of neighboring upstream churn, so conflicts here are almost always "both
     sides added a line" — trivial, keep both.
   - `ruff.schema.json` — generated, binary-ish diff noise. Don't hand-resolve conflicts here:
     after resolving the `.rs` conflicts, delete this file's conflict and regenerate it with
     `cargo dev generate-all` (see verification step below) instead of manually merging JSON.
5. Resolve conflicts file by file. For anything outside the hotspot list above, prefer upstream's
   version unless it directly contradicts our Odoo additions.
6. After conflicts are resolved (`git rebase --continue` or, for the stash-pop path, just
   finishing conflict resolution and `git add`-ing the resolved files):
   - `cargo check --workspace` — confirms the whole workspace still compiles, not just
     `ruff_linter` (registry/codes changes can ripple into `ruff_workspace`, `ruff_dev`, etc.).
   - `cargo dev generate-all` — regenerates `ruff.schema.json` and rule docs; review the diff.
   - The standard test command from `AGENTS.md`:
     ```
     CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_LTO=off INSTA_FORCE_PASS=1 INSTA_UPDATE=always CARGO_PROFILE_DEV_DEBUG="line-tables-only" cargo nextest run -p ruff_linter
     ```
     (fall back to `cargo test -p ruff_linter --lib` if `nextest` isn't installed). Pay special
     attention to `registry::tests::rule_naming_convention` and `registry::tests::linter_sorting`
     — these are exactly the two tests that catch a misplaced `Odoo` variant or a disallowed rule
     name, and they're cheap ways to confirm the registry conflict was resolved correctly.
   - Spot-check the `rules::odoo::tests::*` snapshot tests specifically — a bad conflict resolution
     in a dispatch file (e.g. accidentally dropping an odoo `if checker.is_rule_enabled(...)`
     block) won't fail to compile, it'll just silently stop firing, exactly like the
     `Rule::lint_source()` gotcha documented in `add-odoo-rule`.
7. Run `uv run --only-group dev --locked prek run --files <every file touched by the sync>` (or
   `uvx prek run --files ...` if there's no `uv.lock` in this checkout — pass files in small
   batches of ~5-10; passing dozens of paths in one `--files` invocation has been unreliable here).
8. Do **not** push anywhere yet — report the sync result (commits pulled in, conflicts resolved,
   verification status) and let the user decide when to push `dev` / open the PR to `stb` via the
   `create-pr-mr` skill.

## Usage Examples

### Example 1: Routine sync before starting a new batch of rules

**User:** Before adding more rules, pull in the latest from astral.
**Action:** The agent fetches `astral`, fast-forwards or rebases the working branch, resolves the
expected conflicts in `codes.rs`/`registry.rs`/the checker dispatch files, regenerates
`ruff.schema.json`, re-runs the test suite, and reports what changed upstream — without touching
the `astral` or `stb` remotes.

### Example 2: Sync surfaces an unrelated upstream-worthy fix

**User:** Sync with astral.
**Action:** Mid-conflict-resolution the agent notices an unrelated bug fix opportunity in a file
it's touching. It does not act on it or open anything against `astral` — it finishes the sync and
mentions the observation to the user as a side note.
