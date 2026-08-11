class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(inverse="_set_total")
    subtotal = fields.Float(inverse="_inverse_subtotal")
