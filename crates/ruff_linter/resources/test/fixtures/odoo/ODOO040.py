from odoo import fields, models


class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(compute="_compute_total")
    count = fields.Integer(compute="_compute_count")

    def _compute_total(self):
        self.write({"total": 10})

    def _compute_count(self):
        for record in self.search([]):
            record.write({"count": 1})
        browsed = self.browse(self.ids)
        browsed.write({"count": 2})

    def action_do(self):
        self.write({"state": "done"})
