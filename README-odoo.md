# ruff-odoo

[![PyPI](https://img.shields.io/pypi/v/ruff-odoo?logo=pypi&logoColor=white)](https://pypi.org/project/ruff-odoo/)
[![CI](https://img.shields.io/github/actions/workflow/status/Vauxoo/ruff-odoo/ci.yaml?branch=main&logo=github&label=CI)](https://github.com/Vauxoo/ruff-odoo/actions)
[![Docs](https://img.shields.io/badge/docs-Odoo%20rules-blue)](https://vauxoo.github.io/ruff-odoo/)

`ruff-odoo` is [Vauxoo](https://www.vauxoo.com/)'s fork of [Ruff](https://github.com/astral-sh/ruff),
shipping every upstream linter and formatter rule plus the `OD` and `OAPP` rule groups, ported from
[pylint-odoo](https://github.com/OCA/pylint-odoo) and
[odoo-pre-commit-hooks](https://github.com/OCA/odoo-pre-commit-hooks).

- The Odoo rules, how to enable them, and how to migrate from pylint-odoo:
    <https://vauxoo.github.io/ruff-odoo/>
- Everything else — configuration, the formatter, editor integrations, the upstream rules — behaves
    exactly as upstream and is documented at <https://docs.astral.sh/ruff/>.

This file covers what differs from upstream Ruff for anyone installing or building the fork: why it
exists at all, the command name, the packaging, and the version scheme.

## Why this exists

Vauxoo's Odoo checks used to live in [pylint-odoo](https://github.com/OCA/pylint-odoo) (a Pylint
plugin) and [odoo-pre-commit-hooks](https://github.com/OCA/odoo-pre-commit-hooks), running next to
the autofixing tools already present in the pre-commit stack. Three problems pushed us to port those
checks into Ruff instead, and none of them was something a patch to those projects could have fixed.

**Autofixes corrupted each other.** Every tool rewrote the file on its own, with no idea of what the
others had done to it. A fix applied by one hook routinely invalidated a fix another hook had just
applied, or uncovered a violation that only became reachable after the first rewrite. So the run
ended dirty, the command had to be run a second time, and that run pulled in yet another fix. Getting
a repository to a fixed point — a run that proposes no changes — commonly took three or four passes
of the same command. That is confusing on a developer machine and simply broken in CI, where there is
one run and a dirty tree is a failure.

Ruff does that iteration internally: it lints, applies every non-overlapping fix, re-parses the
result, and repeats until the file stops changing, discarding any fix that would have introduced a
syntax error. One invocation, one stable file.

**Pylint has no autofixes.** Pylint only reports. Any check we also wanted fixed automatically had to
be written twice — once as a Pylint checker that reports it and once as a separate fixer that
rewrites it — with two sets of tests, two notions of what the pattern is, and no guarantee the two
stayed in agreement as they were maintained. In Ruff a fix is attached to the diagnostic that
produced it, so the rule that finds a problem is the same code that repairs it, and its fixture
covers both.

**Speed, which mattered more than anything else.** A linter is only run as often as it is cheap to
run. Pylint infers types through astroid, and a stack of separate hooks pays for a process start and
a fresh parse of every file per tool. Ruff parses each file once, in Rust, in parallel across cores,
and caches unchanged files between runs — fast enough to run on save and on every commit rather than
only in CI, which is what makes the rules actually shape the code being written instead of being
discovered at review time.

### Why a fork, and not a separate repository

The obvious alternative was to keep the Odoo rules in their own project and just depend on Ruff. It
does not work, for reasons that are structural rather than incidental:

- **Ruff has no plugin system.** Rules are compiled into the binary; there is no dynamic loading, no
    stable ABI, and no "register a rule" entry point to call from the outside. That is a deliberate
    design choice upstream — it is part of why a single static binary can dispatch rules with no
    per-rule overhead — not a gap waiting to be filled.
- **A rule is not self-contained.** Adding one touches files that upstream owns: the code-to-rule
    map in `codes.rs`, the linter enum in `registry.rs`, the dispatch inside the AST visitor in
    `checkers/ast/analyze/`, the settings struct behind `lint.odoo`, and the generated JSON schema
    and docs. There is no seam an external crate could attach to.
- **The internal crates are not a public API.** `ruff_linter` describes itself as "an internal
    component crate of Ruff": it is versioned with the binary and refactored freely. A separate
    project built on it would break on close to every upstream release, which is a worse maintenance
    burden than a rebase.
- **A second tool would recreate the original problem.** Even with all of the above solved, shipping
    the Odoo rules as a second binary means a second process, a second parse, a second fix pass over
    the same file, a second configuration file, and a second suppression syntax. The whole point is
    that the Odoo rules run in the same pass as everything else: one command, one AST, one `--fix`,
    one `# noqa`, one cache, one `pyproject.toml`.

The cost of this choice is having to rebase onto upstream Ruff, and that is accepted knowingly. The
fork is arranged to keep that cost low: the Odoo rules live in their own directory, upstream files
are touched as little as possible, and the `ruff` crate and its binary are left untouched behind a
thin wrapper crate.

## The `ruff-odoo` command

The published command is `ruff-odoo`, not `ruff`. It is the exact same CLI, only under a name that
cannot collide with an upstream `ruff` installation, so both can live in the same environment:

```console
$ ruff-odoo check   # Lint all files in the current directory.
$ ruff-odoo format  # Format all files in the current directory.
```

The Python module is `ruff_odoo`, so the CLI is also reachable without the console script:

```console
$ python -m ruff_odoo check
```

## Installation

The package is published to PyPI as [`ruff-odoo`](https://pypi.org/project/ruff-odoo/). It is not
distributed through Homebrew, conda, Docker, or the standalone `astral.sh` installers — those
channels ship upstream Ruff only.

```console
$ # Run it without installing.
$ uvx ruff-odoo check

$ # Install it globally.
$ uv tool install ruff-odoo@latest

$ # Or add it to your project.
$ uv add --dev ruff-odoo

$ # With pip.
$ pip install ruff-odoo
```

Installing `ruff-odoo` alongside the upstream `ruff` package is supported, even in the same
environment: `ruff-odoo` owns the `ruff-odoo` entry point and never touches `ruff`.

## pre-commit

Use the mirror repository, which installs the prebuilt wheels:

```yaml
repos:
  - repo: https://github.com/vauxoo/ruff-pre-commit
    # Use the latest release tag; see the version scheme below.
    rev: 0.16.3.34
    hooks:
      - id: ruff-check
      - id: ruff-format
```

The hook ids match [`astral-sh/ruff-pre-commit`](https://github.com/astral-sh/ruff-pre-commit), so
switching between upstream and this fork means changing only `repo` and `rev`.

This repository also defines the hooks directly (see [`.pre-commit-hooks.yaml`](https://github.com/Vauxoo/ruff-odoo/blob/main/.pre-commit-hooks.yaml)),
but pointing `repo` at it builds Ruff from source with maturin on every environment creation, which
is slow and requires a Rust toolchain. Prefer the mirror.

## Version scheme

Fork releases use **four** components, `x.y.z.w`:

- `x.y.z` is the upstream Ruff release this fork is based on (for example `0.16.2`).
- `w` counts the fork's own releases, independently of that upstream base.

So `0.16.2.4` is the fourth Vauxoo release, built on upstream Ruff `0.16.2`. When the fork is
synced onto a newer upstream, `x.y.z` follows it and `w` carries over untouched: syncing
`0.16.2.4` onto upstream `0.16.3` gives `0.16.3.4`, and the next release is `0.16.3.5`. The
counter never restarts, so a fork version is never reused and never goes backwards.

The version lives in `[project] version` of the root [`pyproject.toml`](https://github.com/Vauxoo/ruff-odoo/blob/main/pyproject.toml)
and is bumped by `bump2version` (see [`.bumpversion.cfg`](https://github.com/Vauxoo/ruff-odoo/blob/main/.bumpversion.cfg)),
which also creates the release tag. Tags are the bare version with no prefix (`0.16.2.4`, not
`v0.16.2.4`); pushing one triggers the PyPI release workflow. Because every fork tag has four
components, it can never be confused with one of upstream's three-component tags.

Cargo cannot express a four-component version, so `crates/ruff_odoo/Cargo.toml` keeps the
three-component `x.y.z` and `crates/ruff/build.rs` reads the real version out of `pyproject.toml`
at build time, exposing it as `RUFF_ODOO_VERSION`. A build made outside a checkout of this
repository falls back to the Cargo version.

### Reporting the version

`--version` prints the bare version and nothing else:

```console
$ ruff-odoo --version
ruff-odoo 0.16.3.34
```

It stays bare on purpose: tools that shell out to the binary (`ruff-lsp`, for one) parse that output
as a [PEP 440](https://peps.python.org/pep-0440/) version and fail on anything appended to it.

The `version` subcommand is the verbose one. It adds the number of commits since the release tag and
the commit the binary was built from:

```console
$ ruff-odoo version
ruff-odoo 0.16.3.34+3 (b45cfcb38 2026-08-15)
```

The leading name is the binary that was actually invoked, so a locally built `ruff` dev binary
introduces itself as `ruff`. A release build sitting exactly on its tag has no `+N` suffix.

For machine-readable output, ask for JSON:

```console
$ ruff-odoo version --output-format json
{
  "version": "0.16.3.34",
  "commit_info": {
    "short_commit_hash": "b45cfcb38",
    "commit_hash": "b45cfcb38d111d9f446b195125290a19e4cf8a4e",
    "commit_date": "2026-08-15",
    "last_tag": "0.16.3.34",
    "commits_since_last_tag": 3
  }
}
```

## Building from source

The `ruff-odoo` binary is a thin wrapper crate (`crates/ruff_odoo`) around the untouched upstream
`ruff` crate; the only differences are the displayed command name and the error prefix. Keeping the
`ruff` crate and its binary unmodified is what keeps rebases onto upstream cheap.

Both binaries build from this checkout, and either one is fine for development:

```sh
cargo run --bin ruff-odoo -- check path/to/file.py
cargo run --bin ruff -- check path/to/file.py
```

See [`AGENTS.md`](https://github.com/Vauxoo/ruff-odoo/blob/main/AGENTS.md) for the full development
workflow, and [`CONTRIBUTING.md`](https://github.com/Vauxoo/ruff-odoo/blob/main/CONTRIBUTING.md) for
upstream's.
