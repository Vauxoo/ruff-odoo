from odoo import models


class MyModel(models.Model):
    _inherit = "my.model"

    def deprecated_in_18(self):
        self.check_access_rights("read")
        self.check_access_rule("write")
        self._filter_access_rules("read")
        self._filter_access_rules_python("read")
        self._check_recursion()
        self._check_m2m_recursion("child_ids")

    def deprecated_in_19(self):
        self.read_group([], ["amount:sum"], ["partner_id"])
        self.check_field_access_rights("read", ["name"])
        self.env["my.model"].browse(1).toggle_active()

    def read_group(self, domain, fields, groupby, offset=0, limit=None, orderby=False):
        """Keeping a deprecated override alive requires delegating to it."""
        return super().read_group(domain, fields, groupby, offset, limit, orderby)

    def toggle_active(self):
        """A `super()` call to a *different* deprecated method is still a migration site."""
        return super().read_group([], [], [])

    def replacements(self):
        self.check_access("write")
        self._filtered_access("read")
        self._has_cycle()
        self._read_group([], ["partner_id"], ["amount:sum"])
        self._check_field_access(self._fields["name"], "read")
        self.action_archive()


class OrdinaryPythonClass:
    def report(self):
        self.read_group([], [], [])
        self.toggle_active()


def module_level_read_group():
    """A plain function call is not an ORM call."""
    return read_group([], [], [])


class LegacyReadGroupKeywords(models.Model):
    _inherit = "sale.order"

    def with_lazy(self):
        # `lazy` is not in the 20.0 signature, so this is unambiguously the pre-20.0 API.
        return self.read_group([], ["amount:sum"], ["partner_id"], lazy=False)

    def with_orderby(self):
        # Same for `orderby`, which 20.0 renamed to `order`.
        return self.read_group([], ["amount:sum"], ["partner_id"], orderby="partner_id")

    def positional_only(self):
        # Indistinguishable from a correct 20.0 call once the name came back, so from 20.0 this
        # one is deliberately not reported.
        return self.read_group([], ["partner_id"], ["amount:sum"])

    def new_api(self):
        # A genuine 20.0 call: groupby then aggregates, and `order`, not `orderby`.
        return self.read_group([], ["partner_id"], ["amount:sum"], order="partner_id")
