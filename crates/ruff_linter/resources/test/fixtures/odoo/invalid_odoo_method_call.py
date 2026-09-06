from odoo import api, fields, models
from odoo.exceptions import UserError
from odoo.http import request


class SaleOrder(models.Model):
    _inherit = "sale.order"

    name = fields.Char()

    def _bad_calls(self):
        # ODE9502: `lazy` and `orderby` are gone from the 20.0 `read_group`.
        self.read_group([], ["amount:sum"], ["partner_id"], lazy=False)
        self.env["sale.order"].read_group([], fields=["amount:sum"], groupby=["partner_id"])
        # ODE9502: `_search` dropped `access_rights_uid` in 18.0.
        self._search([], access_rights_uid=1)
        # ODE9502: `name_search` renamed `args` to `domain` in 19.0.
        request.env["res.partner"].name_search(name="x", args=[])
        # ODE9502: `search` takes a domain plus three optional keywords, not five positionals.
        self.env["res.partner"].search([], 0, 10, "id", True)
        # ODE9502: `load` needs its data.
        self.load(["name"])
        # ODE9502: `default` bound twice.
        self.copy({}, default={})
        # `write` is left alone: this file widens it, so the call may well mean that override.
        self.write({}, from_ui=True)
        # Not reported, by design: seven positionals bind to 20.0's seven parameters even
        # though every one of them lands on a different one. See "Known limitation".
        super().read_group([], ["a:sum"], ["b"], 0, 10, False, True)
        # ODE9502: the chain is still a recordset.
        self.env["res.partner"].sudo().with_context(active_test=False).read_group(
            [], ["id"], ["name"], lazy=True
        )

    def _good_calls(self):
        # The 20.0 signature, spelled correctly.
        self.read_group([], groupby=["partner_id"], aggregates=["amount:sum"])
        self.env["sale.order"]._read_group([], ["partner_id"], ["amount:sum"])
        self.search([], limit=1)
        self.env["res.partner"].search([], offset=0, limit=10, order="id")
        self.with_context(active_test=False).search_count([])
        self.browse([1, 2]).write({"name": "x"})
        self.sudo().unlink()
        # Unpacking hides what is really passed, so nothing is claimed about it.
        self.read_group(*args)
        self.read_group([], ["a"], ["b"], **kwargs)

    def search(self, domain, offset=0, limit=None, order=None, extra=None):
        # This class overrides `search` for `sale.order`, so its own `self.search(...)` is
        # left alone -- but a call routed through another model is not that override.
        self.search([], extra=True)
        # ODE9502: `event.registration` is a different model; `extra` is not a parameter.
        self.env["event.registration"].search([], extra=True)
        # The class declares `sale.order`, so this one really can mean the override.
        self.env["sale.order"].search([], extra=True)
        # Nothing can be told about which model this reaches, so it is left alone too.
        self.env[some_model].search([], extra=True)
        return super().search(domain, offset, limit, order)

    def write(self, vals, from_ui=False):
        # A call to a method this file widens is left alone -- `from_ui` is legal here.
        self.write(vals, from_ui=True)
        # ODE9502: `super()` names the implementation the rule does know, and Odoo's `write`
        # has never taken a `from_ui`.
        super().write(vals, from_ui=from_ui)
        return super().write(vals)

    @api.onchange("name")
    def _onchange_name(self):
        # `api.onchange(...)` above is a decorator call, not a recordset call, even though
        # `onchange` is a `BaseModel` method with a very different signature.
        pass

    def _not_recordsets(self, worksheet, values, client):
        # None of these receivers is a recordset, however much the method names collide.
        worksheet.write(0, 1, "header", self.name)
        values.update(context={}, domain=[])
        client.verifications.create(to="+1", channel="sms")
        UserError.with_traceback(None)


class Wizard(models.TransientModel):
    _name = "sale.wizard"

    def _bad_call(self):
        # ODE9502: reached through `self.env[...]` from a transient model just the same.
        return self.env["sale.order"].read_group([], ["id"], ["name"], lazy=False)


class NotAModel:
    def run(self, data):
        # `self` here is not a recordset, so nothing is checked.
        self.write(data, mode="w")
        self.read_group(data, lazy=True)
