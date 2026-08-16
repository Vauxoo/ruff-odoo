class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(inverse="_set_total")
    subtotal = fields.Float(inverse="_inverse_subtotal")
    grand_total = fields.Float(inverse=f"_set_grand_total")
    net_total = fields.Float(inverse=f"_inverse_net_total")
