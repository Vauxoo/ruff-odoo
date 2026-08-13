class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(search="_find_total")
    subtotal = fields.Float(search="_search_subtotal")
    grand_total = fields.Float(search=f"_find_grand_total")
    net_total = fields.Float(search=f"_search_net_total")
