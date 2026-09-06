# ruff-odoo

[Ruff](https://docs.astral.sh/ruff/) with the Odoo checks from
[pylint-odoo](https://github.com/OCA/pylint-odoo) and
[odoo-pre-commit-hooks](https://github.com/OCA/odoo-pre-commit-hooks) ported to Rust.

This site documents **only the rules this fork adds**. Everything else — configuration,
the formatter, editor integrations, and the ~1000 upstream rules — behaves exactly as
upstream and is documented at [docs.astral.sh/ruff](https://docs.astral.sh/ruff/).

| Group  | Rules | Ported from                                                              |
| ------ | ----- | ------------------------------------------------------------------------ |
| `OD`   | 67    | `pylint-odoo` and `odoo-pre-commit-hooks`                                |
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
    rev: 0.16.3.34
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
extend-select = ["OD", "OAPP"]
```

Or, without a config file:

```shell
ruff-odoo check --preview --select OD,OAPP .
```

Some rules only make sense against a whole Odoo module rather than a single file — the
manifest checks need `__manifest__.py`, and the "file not referenced" checks need the
module's data files — so point Ruff at the addon directory rather than at individual
files.

A few rules are configurable (allowed licenses, allowed categories, required manifest
keys, and so on) under `[tool.ruff.lint.odoo]`; see [settings](settings.md#lintodoo).

## Versioning

Releases use four components, `x.y.z.w`: `x.y.z` is the upstream Ruff release this fork is
built on, and `w` counts the fork's own releases, independently of that base. So `0.16.2.4`
is the fourth Vauxoo release, built on upstream Ruff `0.16.2`. When the fork moves to a newer
upstream, `x.y.z` follows it and `w` carries over untouched — syncing `0.16.2.4` onto
upstream `0.16.3` gives `0.16.3.4`, and the counter never restarts. Release tags are the
bare version, with no `v` prefix — pin `rev:` accordingly.

`--version` prints that version and nothing else, so tools that shell out to the binary and
parse the output as a [PEP 440](https://peps.python.org/pep-0440/) version keep working:

```console
$ ruff-odoo --version
ruff-odoo 0.16.3.34
```

The `version` subcommand is the detailed one, adding the number of commits since the
release tag and the commit the binary was built from. Pass `--output-format json` for the
same information as a machine-readable object:

```console
$ ruff-odoo version
ruff-odoo 0.16.3.34+3 (b45cfcb38 2026-08-15)
```

## Migrating from pylint-odoo

Rule names are unchanged. A check that was `sql-injection` in `pylint-odoo` is
`sql-injection` here too, so existing knowledge, tickets and grep patterns keep working.

Codes are unchanged too, apart from an `OD` prefix that keeps them from colliding with
upstream Ruff's own `C`, `E`, `F`, `R` and `W` codes: `E8103` becomes `ODE8103`, `C8101`
becomes `ODC8101`, `R8180` becomes `ODR8180`. The category letter is part of the code, so
`--select ODC` selects every convention check, `--select ODE` every error, and `--select OD` the whole group.

Two exceptions. The three paid-app checks (`C8117`, `C8118`, `C8119`) live in their own
`OAPP` group, numbered `OAPP001`–`OAPP003`, so that a project can select them separately
from the rest. And the rules with no `pylint-odoo` counterpart — the ports of
`odoo-pre-commit-hooks` checks and the rules invented here — are numbered in an `85xx`
block of their own (`ODC8501`, `ODW8501`, …), which is why no `pylint-odoo` code maps to
them.

Suppression comments do change: Ruff does not read `# pylint: disable`. The
[`pylint-disable-comment`](rules/pylint-disable-comment.md) rule (`ODC8502`) finds the
leftover pragmas and rewrites them, resolving each name — or each old message code such as
`E8102` — to the rule that replaced it. Because the rule names are the ones `pylint-odoo`
already used, the rewrite keeps them, and the result reads the way the pragma did:

```python
env.cr.commit()  # pylint: disable=invalid-commit
env.cr.commit()  # ruff: ignore[invalid-commit]
```

Each pragma has a suppression with the same scope, so nothing widens or narrows: an inline
pragma becomes a trailing `# ruff: ignore[...]`, a `disable-next` becomes an own-line one,
and a block-scoped `disable` becomes a `# ruff: disable[...]` / `# ruff: enable[...]` pair
around that block.

A name only resolves in a suppression comment while preview is on — with preview off, only
codes do. That costs nothing here, since every rule on this site is a preview rule and does
not fire without it either.
