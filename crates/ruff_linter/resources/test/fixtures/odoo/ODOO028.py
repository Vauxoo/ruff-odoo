class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(search="_find_total")
    subtotal = fields.Float(search="_search_subtotal")
