class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(compute="_get_total")
    subtotal = fields.Float(compute="_compute_subtotal")
    grand_total = fields.Float(compute=f"_get_grand_total")
    net_total = fields.Float(compute=f"_compute_net_total")
