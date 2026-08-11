class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(compute="_compute_total")

    def _compute_total(self):
        self.write({"total": 10})

    def action_do(self):
        self.write({"state": "done"})
