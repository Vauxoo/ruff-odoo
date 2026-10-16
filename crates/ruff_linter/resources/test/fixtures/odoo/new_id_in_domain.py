from odoo import api, fields, models
from odoo.fields import Domain


class ResPartner(models.Model):
    _inherit = "res.partner"

    registration_count = fields.Integer(compute="_compute_registration_count")
    order_count = fields.Integer(compute="_compute_order_count")
    other_count = fields.Integer(compute="_compute_other_count")
    saved_count = fields.Integer(compute="_compute_saved_count")
    guarded_count = fields.Integer(compute="_compute_guarded_count")

    # The compute OCA/event#557 fixed: `partner.id` is a `NewId` in a form.
    def _compute_registration_count(self):
        for partner in self:
            partner.registration_count = self.env["event.registration"].search_count(
                [
                    ("attendee_partner_id", "child_of", partner.id),  # error
                    ("state", "not in", ("cancel", "draft")),
                ]
            )

    @api.depends("name")
    def _compute_order_count(self):
        for partner in self.filtered(lambda p: p.active):
            partner.order_count = self.env["sale.order"].search_count(
                [("partner_id", "=", partner.id)]  # error, fix to `in`
            )

    @api.depends_context("company")
    def _compute_other_count(self):
        for record in self.sudo():
            orders = self.env["sale.order"]
            record.other_count = orders.search_count([("partner_id", '!=', record.id)])  # error
            record.other_count += orders.search_count([("partner_id", "not in", record.id)])  # error
            record.other_count += orders.search_count([("partner_id", "in", record.id)])  # error
            record.other_count += orders.search_count([("partner_id", "parent_of", record.id)])  # error
            record.other_count += orders.search_count(Domain("partner_id", "=", record.id))  # error
            # Reported, but no list operator to rewrite it to.
            record.other_count += orders.search_count([("partner_id", ">", record.id)])  # error
            # An operator spelled as an implicit concatenation is reported but not rewritten.
            record.other_count += orders.search_count([("partner_id", "!" "=", record.id)])  # error
        self.other_count = len(
            [p for p in self if self.search_count([("parent_id", "=", p.id)])]  # error
        )

    @api.onchange("parent_id")
    def _onchange_parent_id(self):
        children = self.search([("parent_id", "=", self.id)])  # error
        return {"domain": {"child_ids": [("id", "in", children.ids)]}}

    def _compute_saved_count(self):
        for partner in self:
            # The record's origin, its ids and the id of a related record are not reported.
            partner.saved_count = self.search_count([("parent_id", "=", partner._origin.id)])
            partner.saved_count += self.search_count([("parent_id", "in", partner.ids)])
            partner.saved_count += self.search_count([("user_id", "=", partner.user_id.id)])
            # Not a domain condition: `"b"` is no operator.
            partner.ref = str(("a", "b", partner.id))
        # A loop over records fetched from the database only holds saved records.
        for partner in self.search([]):
            partner.saved_count = self.search_count([("parent_id", "=", partner.id)])
        # `filtered("id")` drops the new records.
        for partner in self.filtered("id"):
            partner.saved_count = self.search_count([("parent_id", "=", partner.id)])

    def _compute_guarded_count(self):
        for partner in self:
            if not partner.id:
                partner.guarded_count = 0
                continue
            partner.guarded_count = self.search_count([("parent_id", "=", partner.id)])
        for record in self:
            record.guarded_count = (
                self.search_count([("parent_id", "=", record.id)]) if record.id else 0
            )

    def action_view_children(self):
        # Not a compute nor an onchange: the record is saved when a button runs.
        return {"domain": [("parent_id", "=", self.id)]}


class NotAModel:
    @api.depends("name")
    def _compute_name(self):
        return [("parent_id", "=", self.id)]
