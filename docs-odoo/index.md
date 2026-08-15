# ruff-odoo

[Ruff](https://docs.astral.sh/ruff/) with the Odoo checks from
[pylint-odoo](https://github.com/OCA/pylint-odoo) and
[odoo-pre-commit-hooks](https://github.com/OCA/odoo-pre-commit-hooks) ported to Rust.

This site documents **only the rules this fork adds**. Everything else — configuration,
the formatter, editor integrations, and the ~1000 upstream rules — behaves exactly as
upstream and is documented at [docs.astral.sh/ruff](https://docs.astral.sh/ruff/).

| Group  | Rules | Ported from                                                              |
| ------ | ----- | ------------------------------------------------------------------------ |
| `ODOO` | 66    | `pylint-odoo`                                                            |
| `OAPP` | 3     | the app-store variants of those checks, which only apply to paid modules |

Start at the [rules index](rules.md).

## Installation

The package is published to PyPI as `ruff-odoo`, because the `ruff` name belongs to
upstream:

```shell
pip install ruff-odoo
```

The binary is still called `ruff`, so this is a drop-in replacement — but for that same
reason it must not be installed next to the upstream `ruff` package in the same
environment.

With pre-commit, use the mirror repo, which installs prebuilt wheels instead of compiling
from source:

```yaml
repos:
  - repo: https://github.com/vauxoo/ruff-pre-commit
    rev: v0.16.2.13
    hooks:
      - id: ruff-check
```

## Usage

Odoo rules are all in [preview](preview.md), so preview mode has to be on — selecting them
without it silently reports nothing:

```toml
[tool.ruff]
preview = true

[tool.ruff.lint]
extend-select = ["ODOO", "OAPP"]
```

Or, without a config file:

```shell
ruff check --preview --select ODOO,OAPP .
```

Some rules only make sense against a whole Odoo module rather than a single file — the
manifest checks need `__manifest__.py`, and the "file not referenced" checks need the
module's data files — so point Ruff at the addon directory rather than at individual
files.

A few rules are configurable (allowed licenses, allowed categories, required manifest
keys, and so on) under `[tool.ruff.lint.odoo]`; see [settings](settings.md#lintodoo).

## Migrating from pylint-odoo

Rule names are unchanged. A check that was `sql-injection` in `pylint-odoo` is
`sql-injection` here too, so existing knowledge, tickets and grep patterns keep working —
only the code changes (`E8103` becomes `ODOO052`).

Suppression comments do change: Ruff honors `# noqa`, not `# pylint: disable`. The
[`pylint-disable-comment`](rules/pylint-disable-comment.md) rule (`ODOO047`) finds the
leftover pragmas and rewrites them, resolving each name — or each old message code such as
`E8102` — to the rule that replaced it.
