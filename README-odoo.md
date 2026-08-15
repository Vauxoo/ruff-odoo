# ruff-odoo

`ruff-odoo` is [Vauxoo](https://www.vauxoo.com/)'s fork of [Ruff](https://github.com/astral-sh/ruff),
shipping every upstream linter and formatter rule plus the `ODOO` and `OAPP` rule groups, ported from
[pylint-odoo](https://github.com/OCA/pylint-odoo) and
[odoo-pre-commit-hooks](https://github.com/OCA/odoo-pre-commit-hooks).

- The Odoo rules, how to enable them, and how to migrate from pylint-odoo:
    <https://vauxoo.github.io/ruff-odoo/>
- Everything else — configuration, the formatter, editor integrations, the upstream rules — behaves
    exactly as upstream and is documented at <https://docs.astral.sh/ruff/>.

This file covers what differs from upstream Ruff for anyone installing or building the fork: the
command name, the packaging, and the version scheme.

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
    rev: 0.16.2.16
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
- `w` counts the fork's own releases on top of that upstream base, starting at `1`.

So `0.16.2.4` is the fourth Vauxoo release built on upstream Ruff `0.16.2`. When the fork is
rebased onto a newer upstream, `x.y.z` follows it and `w` starts counting again.

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
ruff-odoo 0.16.2.16
```

It stays bare on purpose: tools that shell out to the binary (`ruff-lsp`, for one) parse that output
as a [PEP 440](https://peps.python.org/pep-0440/) version and fail on anything appended to it.

The `version` subcommand is the verbose one. It adds the number of commits since the release tag and
the commit the binary was built from:

```console
$ ruff-odoo version
ruff-odoo 0.16.2.16+3 (b45cfcb38 2026-08-15)
```

The leading name is the binary that was actually invoked, so a locally built `ruff` dev binary
introduces itself as `ruff`. A release build sitting exactly on its tag has no `+N` suffix.

For machine-readable output, ask for JSON:

```console
$ ruff-odoo version --output-format json
{
  "version": "0.16.2.16",
  "commit_info": {
    "short_commit_hash": "b45cfcb38",
    "commit_hash": "b45cfcb38d111d9f446b195125290a19e4cf8a4e",
    "commit_date": "2026-08-15",
    "last_tag": "0.16.2.16",
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
