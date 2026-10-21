import re

from odoo import http, models
from odoo.tests import TransactionCase


class OrphanReportRule(models.Model):
    _name = "orphan.report.rule"

    def cron_send_emails(self, template):
        # Error, fixable: vauxoo/ircodoo orphan_management at 18.0.
        template._cache["report_template_ids"] = None
        # Error, fixable: the value the cache format expects for an x2many.
        template._cache["report_template_ids"] = ()

    def _compute_payment_mode(self):
        # Error, fixable: `self` is a record in a model class.
        self._cache["payment_mode"] = "own_account"
        self._cache["payment_mode"] = "company_account"  # trailing comment kept

    def render(self, record, update):
        # Error, fixable: the value is any expression, carried over verbatim.
        update._cache["donor_id"] = record.id if record else False
        # Error, fixable: an attribute chain reads the same twice.
        self.env.user._cache["name"] = "Mitchell"
        # Error, fixable: a bare tuple is wrapped so it stays one argument.
        update._cache["tag_ids"] = 1, 2
        # Error, fixable: the parentheses around the value are kept.
        update._cache["tag_ids"] = (
            1,
            2,
        )
        # Error, fixable: one argument per line when the call does not fit.
        update._cache["a_field_with_a_rather_long_name"] = some_quite_long_function_name(record)

    def _compute_menu(self, menus):
        # Error, no fix: an augmented assignment (odoo/odoo `website` at 18.0).
        for menu in menus:
            menu.parent_id._cache["child_id"] += (menu.id,)
        # Error, no fix: a deletion.
        del menus[0]._cache["child_id"]
        # Error, no fix: the record is a call, written twice by the rewrite.
        self.browse(1)._cache["name"] = "x"
        # Error, no fix: several targets.
        value = self._cache["name"] = "x"
        # Error, no fix: a comment the rewrite would drop.
        self._cache[  # the name
            "name"
        ] = "x"
        # Error, no fix: the mapping methods are gone.
        self._cache.update({"name": "x"})
        self._cache.pop("name")
        self._cache.setdefault("name", "x")
        self._cache.clear()
        self._cache.popitem()
        # Error, no fix: an unpacking target.
        self._cache["name"], other = "x", "y"

    def reads(self, template):
        # OK: reading the cache still works in 19.0.
        name = self._cache["name"]
        name = template._cache.get("name")
        if "name" in self._cache:
            pass
        values = self._convert_to_write(self._cache)
        names = list(self._cache.keys())
        # OK: not `_cache`.
        self._fields["name"]._update_cache(self, "x")
        self.cache["name"] = "x"
        template._cache_other["name"] = "x"


class Portal(http.Controller):
    def update(self, update_sudo, donor_id):
        # Error, fixable: vauxoo/ircodoo mass_mailing_project at 18.0.
        update_sudo._cache["donor_id"] = donor_id


class TestPaymentEnvironment(TransactionCase):
    def test_state(self, payment):
        # Error, fixable: a local in a test.
        payment._cache["state"] = "enabled"
        # Error, fixable: an attribute of `self` in an Odoo class.
        self.partner._cache["name"] = "x"
        # OK: a test case is not a record.
        self._cache["name"] = "x"


class SourceMapGenerator:
    def __init__(self):
        self._cache = {}

    def add(self, source, line):
        # OK: `self` is no record outside an Odoo model (odoo/tools/sourcemap_generator.py).
        self._cache[(source, line)] = "AAAA"
        self.parent._cache["x"] = 1

    @classmethod
    def reset(cls):
        # OK: a class.
        cls._cache["x"] = 1


def clear_route_cache():
    # OK: a module, not a record (odoo/addons/base/tests/test_ir_http.py).
    re._cache.clear()


def post_init_hook(env):
    partner = env.ref("base.main_partner")
    # Error, fixable: a record anywhere.
    partner._cache["name"] = "x"
