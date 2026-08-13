from odoo import _, models


class SaleOrder(models.Model):
    _inherit = "sale.order"

    def action_confirm(self):
        # Flagged: plain literals and pre-translation interpolation.
        self.message_post(body="Order confirmed")
        self.message_post("Order confirmed positionally")
        self.message_post(subject="New quotation", body=_("Translated body"))
        self.message_post(body="Order %s confirmed" % self.name)
        self.message_post(body="Order {} confirmed".format(self.name))
        self.message_post(body=f"Order {self.name} confirmed")
        # Not flagged: translated (interpolating a translation call is ODOO041's job).
        self.message_post(body=_("Order confirmed"))
        self.message_post(body=_("Order %s confirmed") % self.name)
        self.message_post(body="%s - %s" % (_("Order"), _("confirmed")))
        # Not flagged: other keywords and non-literal values.
        self.message_post(body=self.note, message_type="comment")
        self.other_method(body="Not a message_post call")
