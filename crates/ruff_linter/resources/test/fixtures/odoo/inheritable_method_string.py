class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(compute=_compute_total)
    other = fields.Float(compute="_compute_other")

    def _compute_total(self):
        pass

    def _compute_other(self):
        pass
