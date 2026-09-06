from odoo import models
from odoo.http import Controller, request


class SaleOrder(models.Model):
    _inherit = "sale.order"

    def matching(self, domain):
        # Gone in 19.0, and nothing else in the ORM answers to the name.
        return self._where_calc(domain)

    def labels(self):
        # Gone in 18.0, so reported on 18.0 and on everything after it.
        return self.name_get()

    def rebuild(self):
        # 19.0 kept a module level `_setup_fields`, which does not make the call work.
        self._setup_fields()
        self.clear_caches()

    def still_there(self, domain):
        # Every one of these survives, whatever the version.
        return self.search(domain).sudo().exists()

    def on_a_field(self, domain):
        # `_condition_to_sql` left `BaseModel` in 19.0 and arrived on `Field`, so the
        # generated removal set leaves it out and this stays correct code.
        return self._fields["state"]._condition_to_sql(domain, "state", self)


class SaleOrderWithOwnLabels(models.Model):
    _inherit = "sale.order"

    def name_get(self):
        return [(record.id, record.display_name) for record in self]

    def labels(self):
        # The class defines the name, so the call reaches that and not the ORM.
        return self.name_get()


class CustomerPortal(Controller):
    def orders(self, domain):
        sale_obj = request.env["sale.order"]
        # The shape a migration leaves behind: the model in a local, in a controller.
        return sale_obj._where_calc(domain)


class Helper:
    def run(self, partner):
        # Not Odoo: a class inheriting nothing at all is Python, whatever it calls.
        partner.name_get()


class PlainHelper(object):
    def run(self, partner):
        partner.name_get()


class OrderError(Exception):
    def report(self, partner):
        partner.name_get()


def free_function(partner):
    # Outside any class there is no telling what the receiver is.
    partner.name_get()


class ReportParser(models.AbstractModel):
    _name = "report.sale.order"

    def render(self, partner):
        # A removed method on a plain local, inside an Odoo class: still reported.
        return partner.name_get()
