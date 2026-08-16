from odoo import _, exceptions, models
from odoo.exceptions import UserError, ValidationError


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
        # Not flagged: translated (interpolating a translation call is ODW8115's job).
        self.message_post(body=_("Order confirmed"))
        self.message_post(body=_("Order %s confirmed") % self.name)
        self.message_post(body="%s - %s" % (_("Order"), _("confirmed")))
        # Not flagged: other keywords and non-literal values.
        self.message_post(body=self.note, message_type="comment")
        self.other_method(body="Not a message_post call")

    def action_cancel(self):
        # Flagged: the message of a raised Odoo exception, imported directly...
        raise UserError("Order cannot be cancelled")

    def action_draft(self):
        # ...reached through the module...
        raise exceptions.ValidationError("Order cannot go back to draft")

    def action_lock(self):
        # ...and interpolated before translation.
        raise UserError("Order %s cannot be locked" % self.name)

    def action_unlock(self):
        raise ValidationError("Order {} cannot be unlocked".format(self.name))

    def action_done(self):
        raise UserError(f"Order {self.name} cannot be marked done")

    def action_reset(self):
        # Not flagged: the message is already translated.
        raise UserError(_("Order cannot be reset"))

    def action_send(self):
        # Not flagged: translated before interpolation.
        raise UserError(_("Order %s cannot be sent") % self.name)

    def action_quote(self):
        # Not flagged: the message is not a literal.
        raise UserError(self.cancel_reason)

    def action_invoice(self):
        # Not flagged: not an Odoo exception.
        raise ValueError("Not an Odoo exception")

    def action_ship(self):
        # Not flagged: no message at all.
        raise UserError()

    def action_retry(self):
        try:
            self.action_ship()
        except UserError:
            # Not flagged: a bare re-raise carries no new message.
            raise
