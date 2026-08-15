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

The command it installs is `ruff-odoo`, not `ruff`. It is the same CLI with the same
subcommands and the same configuration, only under a name that cannot collide with an
upstream `ruff` installation, so both can live in the same environment:

```shell
ruff-odoo check .
```

The Python module is `ruff_odoo`, so `python -m ruff_odoo check .` works too.

With pre-commit, use the mirror repo, which installs prebuilt wheels instead of compiling
from source:

```yaml
repos:
  - repo: https://github.com/vauxoo/ruff-pre-commit
    rev: 0.16.2.14
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
ruff-odoo check --preview --select ODOO,OAPP .
```

Some rules only make sense against a whole Odoo module rather than a single file — the
manifest checks need `__manifest__.py`, and the "file not referenced" checks need the
module's data files — so point Ruff at the addon directory rather than at individual
files.

A few rules are configurable (allowed licenses, allowed categories, required manifest
keys, and so on) under `[tool.ruff.lint.odoo]`; see [settings](settings.md#lintodoo).

## Versioning

Releases use four components, `x.y.z.w`: `x.y.z` is the upstream Ruff release this fork is
built on, and `w` counts the fork's own releases on top of it. So `0.16.2.4` is the
fourth Vauxoo release of upstream Ruff `0.16.2`. When the fork moves to a newer
upstream, `x.y.z` follows it and `w` starts counting again. Release tags are the bare
version, with no `v` prefix — pin `rev:` accordingly.

`--version` prints that version and nothing else, so tools that shell out to the binary and
parse the output as a [PEP 440](https://peps.python.org/pep-0440/) version keep working:

```console
$ ruff-odoo --version
ruff-odoo 0.16.2.14
```

The `version` subcommand is the detailed one, adding the number of commits since the
release tag and the commit the binary was built from. Pass `--output-format json` for the
same information as a machine-readable object:

```console
$ ruff-odoo version
ruff-odoo 0.16.2.14+3 (b45cfcb38 2026-08-15)
```

## Migrating from pylint-odoo

Rule names are unchanged. A check that was `sql-injection` in `pylint-odoo` is
`sql-injection` here too, so existing knowledge, tickets and grep patterns keep working —
only the code changes (`E8103` becomes `ODOO052`).

Suppression comments do change: Ruff honors `# noqa`, not `# pylint: disable`. The
[`pylint-disable-comment`](rules/pylint-disable-comment.md) rule (`ODOO047`) finds the
leftover pragmas and rewrites them, resolving each name — or each old message code such as
`E8102` — to the rule that replaced it.
