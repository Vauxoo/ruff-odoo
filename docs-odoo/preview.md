# Preview

Every `ODOO` and `OAPP` rule is in **preview**, Ruff's opt-in state for rules that are
still allowed to change. Preview rules are never enabled by default, and selecting one
without turning preview on has no effect — which is the most common reason for "I selected
`ODOO` and nothing was reported".

Turn it on in `pyproject.toml`:

```toml
[tool.ruff]
preview = true
```

or per invocation:

```shell
ruff check --preview --select ODOO,OAPP .
```

While a rule is in preview, its diagnostic message, the exact code it flags, and the fix
it offers may change between releases. What will *not* change is its name: each rule keeps
the name of the `pylint-odoo` or `odoo-pre-commit-hooks` check it was ported from, so
`# pylint: disable=<name>` maps one-to-one onto the Ruff rule.

For how preview works in Ruff generally, see
[the upstream preview documentation](https://docs.astral.sh/ruff/preview/).
