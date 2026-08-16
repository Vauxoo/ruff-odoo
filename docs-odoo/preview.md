# Preview

Every `OD` and `OAPP` rule is in **preview**, Ruff's opt-in state for rules that are
still allowed to change. Preview rules are never enabled by default, and selecting one
without turning preview on has no effect — which is the most common reason for "I selected
`OD` and nothing was reported".

Turn it on in `pyproject.toml`:

```toml
[tool.ruff]
preview = true
```

or per invocation:

```shell
ruff-odoo check --preview --select OD,OAPP .
```

While a rule is in preview, its diagnostic message, the source constructs it flags, and the
fix it offers may change between releases. What will *not* change is its identity: each
rule keeps the name of the `pylint-odoo` or `odoo-pre-commit-hooks` check it was ported
from, so `# pylint: disable=<name>` maps one-to-one onto the Ruff rule, and its code is
that check's own message id under an `OD` prefix (`E8103` → `ODE8103`), so suppression
comments are just as stable whichever of the two you write.

Preview is also what lets a suppression name a rule — `# ruff: ignore[sql-injection]`
rather than `# ruff: ignore[ODE8103]`. Since these rules need preview anyway, both forms
work wherever they are relevant, and the migration rule writes the name.

For how preview works in Ruff generally, see
[the upstream preview documentation](https://docs.astral.sh/ruff/preview/).
