# Ruff Repository

This repository contains both Ruff (a Python linter and formatter) and ty (a Python type checker). The crates follow a naming convention: `ruff_*` for Ruff-specific code and `ty_*` for ty-specific code. ty reuses several Ruff crates, including the Python parser (`ruff_python_parser`) and AST definitions (`ruff_python_ast`).

## Before contributing

Before starting work on an issue and again before opening a pull request, follow the
[guidance on avoiding duplicate work](CONTRIBUTING.md#avoiding-duplicate-work). If an open pull
request already addresses the issue, do not submit a competing one without maintainer agreement.

## PR conventions

Before opening a PR, inspect the repository's available GitHub labels and their descriptions,
excluding archived labels. Also inspect the current Rooster configuration, including required and
ignored labels, in the `[tool.rooster]` and
`[tool.rooster.section-labels]` sections of [Ruff's `pyproject.toml`](pyproject.toml) and
[ty's `pyproject.toml`](https://github.com/astral-sh/ty/blob/main/pyproject.toml). Labels such as
`internal`, `testing`, and `ci` can exclude a PR from a changelog. Decide whether and how the change
should appear in each changelog, then select appropriate labels for the PR's content.

When working on ty, PR titles should start with `[ty]`. If you can, add the `ty` GitHub label.

If you have permission, apply the selected labels when creating the PR or afterward, then verify
that the PR's actual labels include them and have the intended effect on each changelog. If the
available labels or a Rooster configuration could not be inspected, say so when reporting the PR.

## Code Review Rules

For security reviews of Ruff and ty runtime changes, use the
[threat models](agents/references/threat-models.md) to assess trust boundaries and
calibrate severity.

When reviewing a branch or pull request, be deliberately nitpicky. Report not
only bugs and regressions, but also architectural and maintenance risks, weak
test coverage, unclear code, unnecessary complexity, and meaningful style or
consistency issues. Order findings by severity, cite files and lines, and
distinguish blockers from non-blocking improvements. Number each review point
for easy reference in subsequent review discussion.

During code review, check the proposed changes against all applicable code, test,
documentation, and architectural conventions in this `AGENTS.md`. Report
meaningful violations introduced by the changes; do not apply agent-only workflow
instructions to PR authors or flag unrelated pre-existing issues.

For security reviews of repository, CI, and release changes, use the
[repository threat model](agents/references/repository-threat-model.md) to assess
trust boundaries and calibrate severity.

## Writing for human readers

Write every mdtest paragraph, code comment, piece of documentation, PR description, and GitHub issue for its eventual reader, not for the current Codex conversation. Determine the reader's knowledge, purpose, and likely questions privately; do not add an audience-analysis section to the artifact.

Base public-facing text on the final code, complete final diff, relevant issue, and verified evidence. Explain the observable behavior or problem before implementation details. Use established technical terminology and the actual names of types, functions, files, and invariants.

Do not mention discarded alternatives, intermediate edits, private instructions, tool usage, branch or draft status, local test commands, or session history unless the reader needs that information to understand the final result. Do not describe reverted changes as part of the final change.

Preserve useful human-written comments unless the code change makes them inaccurate. Before finishing, reread each changed artifact as someone with no access to the Codex session and remove or explain anything that would surprise or confuse that reader.

## Running Tests

File-watcher tests rely on operating-system events that some execution environments do not deliver. Do not run tests that depend on these events in a Codex sandbox or another sandbox where these events may not be delivered; for example, the `ty` crate's `file_watching` integration tests can time out waiting for them. The commands below exclude that test binary. It is usually unnecessary to run file-watcher tests locally before filing a change unless you are certain that the change affects file-watching behavior. To run the `file_watching` binary, use an environment that delivers file-watcher events and run `cargo nextest run -p ty --test file_watching` or `cargo test -p ty --test file_watching`, using the snapshot environment variables shown below.

Run the workspace tests except `file_watching` (using `nextest` for faster execution and setting `INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1` to ensure all snapshots are updated):

```sh
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo nextest run --workspace -E 'not binary(=file_watching)'
```

Run tests for a specific crate. Keep the filter when substituting another crate, including `ty`:

```sh
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo nextest run -p ty_python_semantic -E 'not binary(=file_watching)'
```

Run a single mdtest file. The path to the mdtest file should be relative to the `crates/ty_python_semantic/resources/mdtest` folder. Include `--test mdtest` to avoid building unrelated test binaries:

```sh
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo nextest run -p ty_python_semantic --test mdtest -- mdtest::<path/to/mdtest_file.md>
```

To run a specific mdtest within a file, use a substring of the Markdown header text as `MDTEST_TEST_FILTER`. Only use this if it's necessary to isolate a single test case:

```sh
MDTEST_TEST_FILTER="<filter>" INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo nextest run -p ty_python_semantic --test mdtest -- mdtest::<path/to/mdtest_file.md>
```

### Fallback without nextest

If `cargo nextest` is not available, use `cargo test` with the same environment variables. For the workspace, exclude `ty` and then run its other test targets explicitly. If new non-watcher test targets are added to `ty`, add them to the `ty` command below; `cargo metadata --no-deps --format-version 1` lists each crate's targets:

```sh
# Run workspace tests outside the ty crate.
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo test --workspace --exclude ty

# Run ty tests except file_watching.
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo test -p ty --lib --bins --test cli --test self_environment
```

To run tests for one crate, use the `ty` command above for `ty`; for other crates, use:

```sh
# Run tests for a specific crate.
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo test -p ty_python_semantic

# Run a single mdtest file.
INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo test -p ty_python_semantic --test mdtest -- <path/to/mdtest_file.md>

# Run a specific mdtest within a file.
MDTEST_TEST_FILTER="<filter>" INSTA_FORCE_PASS=1 INSTA_UPDATE=always MDTEST_UPDATE_SNAPSHOTS=1 cargo test -p ty_python_semantic --test mdtest -- <path/to/mdtest_file.md>
```

### Snapshot updates

After running the tests, always review the contents of any snapshots that have been added or updated.

When running tests with `INSTA_FORCE_PASS=1`, check for `.pending-snap` files if any affected tests use inline snapshots.

Never edit snapshot files or inline snapshot bodies manually. Regenerate them by running the relevant tests with the snapshot-update environment variables documented above, then review the generated diff.

## Writing mdtests

- Write mdtests as readable, literate specifications, and minimize the context a reader must hold in mind. Prefer short, focused code blocks, and define types, fixtures, and helpers close to the assertions that use them. Sections and subsections can be long when they develop a coherent topic through many short examples interspersed with prose. Do not split a subsection, or flag it in review, solely because of its length. Give independent scenarios separate sibling Markdown test headings at the same level; only introduce child headings if any existing code beneath their parent is first moved into child sections. When scenarios need shared setup, interleave short prose-and-code blocks under the same heading.
- Code blocks for the same file within a section are concatenated into one file. For inline snapshots, keep each example's code block close to its `# snapshot` block. Ideally, any given code block only has one snapshot in it, but one codeblock containing several snapshots is also acceptable. Repeating a short, independent function definition, including one with the same name, is fine if it keeps the code triggering a snapshot close to the snapshot demonstrating the expected diagnostic on that code. Reuse shared setup when clearer, and check that repeated names do not affect other examples.
- When a Python statement in an mdtest is included only or primarily to check that ty does not emit a diagnostic for it, normally add a `# no diagnostic` comment on or above the statement. Statements used as setup for later assertions do not need such comments.
- Prioritize document structure and readability over avoiding duplicated setup. Add a test to an existing section when its heading accurately describes the new scenario, adding or improving introductory prose as needed; otherwise, create a separate sibling section, even if that requires repeating a small fixture.
- Order mdtests from basic, common behavior to more specialized cases. Place narrow regression tests alongside closely related examples when they fit naturally; otherwise, put them near the end of the relevant section or file. Do not put an obscure special case at the beginning simply because it is the newest regression.
- Aim for readable documents that serve as both documentation and tests. Use prose to explain the key type checker behavior and its rationale where helpful. A sentence or fragment may suffice; avoid repeating what the heading, surrounding explanation, or assertions already make clear. Use clear, precise terminology. Avoid using jargon where it's unnecessary, and avoid inventing new jargon if there's an existing term of art used in that file. Avoid long paragraphs covering multiple scenarios followed by a single long code block.
- Prefer examples whose purpose is understandable without following issue links. For narrow regressions, a brief issue reference can be enough when additional explanation would merely restate the code or obscure the relevant behavior.
- Prefer direct, present-tense descriptions of behavior over abstract requirements: write "We reject this assignment" rather than "We must reject this assignment."
- Keep each code example as simple as possible while still demonstrating the key behavior. When adapting real-world code or an issue reproducer, remove incidental types, methods, type parameters, imports, and domain-specific details. Preserve complexity only when necessary to reproduce the regression or distinguish the intended behavior, and reuse nearby fixtures or simple built-in types when doing so keeps the test easy to understand.
- Prefer a minimal, purpose-built custom type over a standard-library type when a regression depends on particular attributes, methods, bounds, or constraints. Define the relevant behavior in the test so readers do not need to look up the standard-library type to understand the scenario. For commonly used standard-library types, consider adding a separate regression using the real type to protect against changes in typeshed.
- Place each mdtest in a file for the behavior it actually tests, and assert that behavior directly. Prefer an existing file when one already covers that behavior; create a new file when no existing file is a good fit. Do not choose a file solely because its directive or helper can express the assertion.
- When testing generic behavior supported by both legacy and PEP 695 syntax, add equivalent cases under `generics/legacy/` and `generics/pep695/`.

## Running Clippy

```sh
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features
```

## Running Debug Builds

Use debug builds (not `--release`) when developing, as release builds lack debug assertions and have slower compile times.

Run Ruff:

```sh
cargo run --bin ruff -- check path/to/file.py
```

Run ty:

```sh
cargo run --bin ty -- check path/to/file.py
```

## Working on ty

The guidance in this section applies to edits to `ty*` crates, reviews of ty PRs, or other work when the ty type checker has been specifically mentioned by the user.

### Related skills

When the task matches a more specific ty workflow, also read and follow that skill from the repository root:

- Adding new ty rules, changing diagnostics or diagnostic messages, or reviewing diagnostics: `.agents/skills/adding-ty-diagnostics/SKILL.md`.
- Ecosystem report summaries: `.agents/skills/summarise-ecosystem-results/SKILL.md`.
- Reproducing, investigating, or minimizing ecosystem or primer differences: `.agents/skills/minimizing-ty-ecosystem-changes/SKILL.md`.

### Completion ranking

When changing ty autocomplete ranking, add or update evaluation fixtures under `crates/ty_completion_eval/truth/`. Extend an existing project when it is a good fit for the behavior being tested; otherwise, add a new one. Use `<CURSOR:expected_name>` directives to assert ranking, and include the expected module for auto-import completions. Add `completion.rs` unit tests only when the evaluation fixtures cannot adequately cover the behavior.

Regenerate and review the committed evaluation results after changing ranking behavior or fixtures:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_LTO=off CARGO_PROFILE_DEV_DEBUG="line-tables-only" cargo run --package ty_completion_eval -- all --threshold 0.4 --tasks crates/ty_completion_eval/completion-evaluation-tasks.csv
```

To inspect one evaluation task, run `cargo run --package ty_completion_eval -- show-one <fixture-name> --file-name <file-name> --index <cursor-index>`.

### Ad hoc reproductions

When running ty against a temporary Python reproduction file, create it outside the Ruff checkout (for example, under `/tmp`). A file inside the checkout discovers Ruff's root `pyproject.toml`, whose `requires-python = ">=3.7"` causes ty to infer Python 3.7 as the default Python version.

### The `db` parameter

For free functions and associated functions without a `self` parameter, `db` should be the first parameter. For methods with a `self` parameter, `db` should come immediately after `self`.

### Salsa tips

#### Tracked functions and methods

Adding `#[salsa::tracked]` to a function or method means that the Salsa framework will cache the function/method.
This can sometimes be done for performance reasons, and can also be done to ensure incremental computation in an
IDE context.

Methods that access `.node()` should usually be `#[salsa::tracked]`, or ty's incrementality will suffer:
we don't want to accidentally introduce a dependency on module `a`'s AST in a Salsa query that would be
called when type-checking module `b`. Prefer higher-level semantic APIs over raw AST access where possible,
but ask for guidance from the user if this would require significant refactoring.

#### Reduce memory usage where possible

For Salsa-cached values, avoid retaining excess collection capacity. Prefer boxed slices; otherwise shrink collections that may have spare capacity before returning them. In particular, inspect `HashMap` and `HashSet` values constructed via `extend`, `collect`, explicit reservation, or removal, since those operations can leave capacity that insert-only construction does not.

Salsa caching can occur due to a function/method having `#[salsa::tracked]` on it, or due to a struct with `#[salsa::interned]` being constructed.

## Working on the ODOO plugin

The guidance in this section applies to edits to the Vauxoo-specific `OD` rule group under `crates/ruff_linter/src/rules/odoo/`, or other work when it has been specifically mentioned by the user.

### Related skills

Read `.agents/skills/add-odoo-rule/SKILL.md` before any edit under
`crates/ruff_linter/src/rules/odoo/`, whether the task adds a rule or changes one that
already exists, and without waiting to be asked for it by name. It carries the per-rule
checklist — the `codes.rs` numbering blocks, which dispatch file a check belongs in, the
`registry.rs` `lint_source` arm whose absence makes a token rule silently report nothing,
and the verification list to run before calling a rule done. Skipping it produces rules that
compile and never fire.

Two things make it easy to miss, so do not wait for the tooling to surface it:

- It is not installed under `~/.claude/skills/`. It ships as a repository plugin
    (`.agents/.claude-plugin/`, marketplace `ruff-agent-skills`), and `.claude/settings.json`
    enables only `ty-skills`, so the skill never appears in the agent's skill listing. Read
    the `SKILL.md` file directly instead of trying to invoke it by name.
- A session whose working directory is not this checkout — a scratch context directory, say
    — does not load this `AGENTS.md` either. When work targets this repository, read both
    files by path.

The other two apply when the task matches them, and follow the same "read the file from the
repository root" rule:

- Opening a Pull Request on GitHub or a Merge Request on GitLab/git.vauxoo.com for this repo: `.agents/skills/create-pr-mr/SKILL.md`.
- Rebasing/syncing this fork onto the latest astral-sh/ruff upstream: `.agents/skills/sync-astral-upstream/SKILL.md`.

### Documentation site

The `OD`/`OAPP` rules are published to <https://vauxoo.github.io/ruff-odoo/> from
`docs-odoo/`, which is a separate mkdocs site from upstream's `docs/`. Its sources are
generated by `cargo dev generate-odoo-docs` (and therefore by `cargo dev generate-all`,
which every rule change already requires), so a rule's `///` doc comment is the only place
its documentation is written. Only `docs-odoo/index.md` and `docs-odoo/preview.md` are
hand-written and checked in; `rules.md`, `settings.md` and `rules/*.md` are generated and
gitignored.

To review the site locally, generate it and then build or serve it. `mkdocs` is not a
development dependency of this checkout — `uv run --only-group dev` does not provide it and
a bare `mkdocs` is "command not found" unless you happen to have one on `PATH`. Take the
pinned set the `mkdocs (odoo)` CI job installs, so that a local build fails on the same
things CI does:

```sh
uv run --no-project --with-requirements docs/requirements.txt mkdocs build --strict -f mkdocs-odoo.yml
```

`--no-project` is what keeps `uv run` from building Ruff from source first. Always use
`--strict`: it is what turns a broken link or a missing anchor in a doc comment into an
error, and it is what that CI job runs.

Two constraints are easy to break without noticing:

- Rule pages link to `../rules.md`, `../preview.md` and `../settings.md` with paths
    hardcoded in `crates/ruff_dev/src/generate_docs.rs`, so `docs-odoo/` has to keep those
    file names.
- `docs-odoo/settings.md` documents only the `lint.odoo` option group. A rule that
    references an option outside that group in its `## Options` section will produce a
    dangling link and fail the build; either document the option there or don't reference it.

### Rule names and codes must match pylint-odoo

Every `OD`/`OAPP` rule keeps the exact name of the pylint-odoo or odoo-pre-commit-hooks
check it was ported from, so that a `# pylint: disable=<name>` maps one-to-one onto the Ruff
rule and users can keep using the names they already know. Some of those names — `use-vim-comment`,
`prefer-env-translation`, `consider-merging-classes-inherited`, `translation-positional-used` —
violate upstream Ruff's naming convention, so this fork deliberately keeps
`crates/ruff_linter/resources/test/disallowed_rule_names.txt` **empty** and makes
`registry::tests::rule_naming_convention` skip blank lines. Both are astral-owned files: after a
`sync-astral-upstream` rebase, re-check that the list is still empty and that renaming pressure
from upstream has not crept back in.

The codes follow the same rule. The `Odoo` linter's prefix is `OD` and the rest of the code is
the pylint-odoo message id, category letter included, so `E8103 sql-injection` is `ODE8103` —
the same shape upstream uses for Pylint (`PL` + `C0414`). Because the letter is inside the code,
`--select ODC` selects every convention check and `--select OD` the whole group. Three
consequences worth knowing before adding or renumbering a rule:

- A rule ported from pylint-odoo takes that project's code verbatim. Codes are therefore not
    sequential and have gaps wherever pylint-odoo has one; do not "tidy" them.
- A rule with no pylint-odoo counterpart gets a number in one of the fork's own blocks, under
    the matching letter: `85xx` for a port of an odoo-pre-commit-hooks check, `95xx` for a rule
    invented here (`m2m-relation-is-label` is `ODW9501`). Keeping those out of pylint-odoo's
    ranges is what guarantees a future pylint-odoo message can't collide with one of ours, and
    keeping the two apart leaves `85xx` free for whatever else arrives from upstream projects.
- The three paid-app rules keep the plain `OAPP001`–`OAPP003` sequence instead of their
    pylint-odoo ids (`C8117`–`C8119`), because they live in a separate linter so that selecting
    `OD` doesn't drag them in, and those ids only mean anything inside the `OD` numbering.

Fixtures and snapshots are named after the rule, not its code
(`fixtures/odoo/sql_injection.py`), so a code revision doesn't move every test file.

`pylint-disable-comment` (`ODC8502`) maps old pylint-odoo message codes and names onto Ruff
rules. Its `pylint_odoo_messages_are_all_mapped` test enumerates every pylint-odoo message, so
porting a new check means adding its code to `MESSAGE_ALIASES` and its `(code, name)` pair to
that test's list.

### The distributed binary and the fork version

The published CLI is `ruff-odoo`, built from the thin `crates/ruff_odoo` wrapper around the untouched `ruff` crate so the PyPI package never fights upstream `ruff` for its entry point. Keep `crates/ruff/src/main.rs` and `crates/ruff_odoo/src/main.rs` in sync; only the displayed command name and the hard-error prefix differ. The dev binary (`cargo run --bin ruff`) is still the one to use for local work.

Fork releases use a four-component version, `x.y.z.w`, where `x.y.z` is the upstream base and `w` counts fork-only releases. Cargo cannot hold four components, so `pyproject.toml` is the source of truth (bumped by `bump2version`) and `crates/ruff/build.rs` exposes it as `RUFF_ODOO_VERSION` for `crates/ruff/src/version.rs`. `--version` must stay a bare version number — tools such as `ruff-lsp` parse it as a PEP 440 version — while the `version` subcommand carries the commit information. Two hand-written documents cover this for users and both have to be updated together when any of it changes: `README-odoo.md`, which is also the PyPI description, and the `docs-odoo/index.md` page of the documentation site.

Both are listed in `.bumpversion.cfg` without a `search` key, so a release rewrites **every** occurrence of the version in them — that is what keeps the pre-commit `rev:` users copy and the sample `--version` output current. Two consequences when editing either file: a version used to illustrate the `x.y.z.w` scheme has to differ from the current release or it gets rewritten as if it were the release, and each file has to keep mentioning the current version or the bump fails outright. Don't document any of this in `.bumpversion.cfg` itself; `bump2version` rewrites that file through `configparser` on every release and silently drops comments.

## Generated Release Workflow

Parts of `.github/workflows/release.yml` are generated by cargo-dist from `dist-workspace.toml`. Before editing the release workflow, check whether the relevant section is generated. Prefer changing `dist-workspace.toml` or the referenced reusable workflow instead of editing generated YAML. After modifying cargo-dist configuration, regenerate the workflow with the cargo-dist version pinned in `dist-workspace.toml` and inspect the resulting diff to ensure the change will survive future regenerations.

What actually publishes `ruff-odoo` is `publish-odoo.yml`, a fork-only workflow that calls the
hand-written `build-binaries.yml` and `publish-pypi.yml` directly, without cargo-dist. Every
`runs-on` in `build-binaries.yml` therefore has to keep its
`github.repository == 'astral-sh/ruff' && '<upstream runner>' || '<github-hosted>'` fallback:
upstream's Depot and Namespace runners exist only in its organization, and a job asking for a
label no runner offers **queues forever rather than failing**, so a release stalls silently with
nothing red to explain it. That is how `0.16.3.17` and `0.16.3.18` were tagged but never reached
PyPI. After a `sync-astral-upstream` rebase, re-check every `runs-on` in that file, and confirm
a release actually published rather than assuming the tag was enough.

The same fallback belongs on the dist-generated jobs of `release.yml`, which take their runner
from `[dist.github-custom-runners]` in `dist-workspace.toml`. It is not only the release that
suffers when it is missing: `pr-run-mode = "plan"` runs `Release / plan` on every pull request,
so a bare Depot label leaves a check queued on each one, waiting on a runner that does not exist
in this organization. The conditional lives in both files — the workflow, which is what runs
today because `allow-dirty = ["ci"]` keeps dist from regenerating it, and the dist configuration,
so a regeneration reproduces it instead of restoring the bare label.

## Development Guidelines

- All significant changes must be tested. Add or update focused tests for semantic changes when existing coverage does not already establish the intended behavior.
- Look to see if your tests could go in an existing file before adding a new file for your tests.
- Get your tests to pass. If you didn't run the tests, your code does not work.
- Follow existing code style. Check neighboring files for patterns.
- Prefer narrow visibility by default because this workspace is generally its own consumer. However, do not add workarounds solely to avoid `pub`: make an item public when another workspace crate needs it and that produces the cleaner implementation.
- Rust imports should always go at the top of the file, never locally in functions.
- Run `uv run --only-dev --locked prek` at the end of a task if you changed files in the repo. This includes changes such as rebases or addressing review comments. Use `uv run --only-dev --locked prek run --files <path1> <path2>` and pass every file you changed. This keeps the hook run independent of staged state and avoids sweeping unrelated changes. Use `uv run --only-dev --locked prek run --all-files` when a full-repository hook sweep is specifically needed.
- Before writing significant amounts of new code, look for existing utilities or mechanisms that could solve the problem. Avoid expanding the task to unrelated issues, but do not confuse keeping the task focused with minimizing the size of the implementation. Prefer addressing the underlying architectural problem over adding a localized workaround, even when doing so requires a substantial refactor or rearchitecture. Ask the user for guidance if in doubt about whether to attempt a larger refactor or not.
- Try hard to avoid patterns that require `panic!`, `unreachable!`, `.unwrap()` or `.expect()`. Instead, try to encode those constraints in the type system. Don't be afraid to write code that's more verbose or requires largeish refactors if it enables you to avoid these unsafe calls.
- Prefer let chains (`if let` combined with `&&`) and let guards (`PAT if let ... =>`) over nested `if let` statements to reduce indentation and improve readability. At the end of a task, always check your work to see if you missed opportunities to use `let` chains or `let` guards.
- If you _have_ to suppress a Clippy lint, prefer to use `#[expect()]` over `[allow()]`, where possible. But if a lint is complaining about unused/dead code, it's usually best to just delete the unused code.
- Don't use comments to narrate code, but do use them to explain invariants and why something unusual was done a particular way. Make sure that a comment will make sense to somebody who's reading the code for the first time. Prefer plain language, avoid jargon, and don't be afraid to be more verbose if it's necessary to explain something well. Giving examples of the kind of Python code we're trying to model at this particular point in Ruff or ty can often be very helpful for future readers of the code.
- Run `cargo dev generate-all` after changing configuration options, CLI arguments, lint rules, or environment variable definitions, as these changes require regeneration of schemas, docs, and CLI references.
- Don't prefix tests with `test_`.
- Don't separate struct definitions from their `impl` blocks unless the `impl` is deliberately placed in a separate file, as for large structs.
- Write all self-contained Python scripts as PEP 723 scripts, with inline metadata.
- When running a PEP 723 script, run it with `uv run <script>`. If a script doesn't have PEP 723 metadata, run it with `--no-project`, `--script`, or similar to avoid building Ruff from source, which is very slow and usually unnecessary.
