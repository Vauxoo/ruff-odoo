class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(compute="_get_total")
    subtotal = fields.Float(compute="_compute_subtotal")
