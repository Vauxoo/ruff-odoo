class MyModel(models.Model):
    _inherit = "my.model"

    amount = fields.Float(digits_compute=precision)
    ref = fields.Char(select=True)
    name = fields.Char(index=True)
    # Flagged, but no fix: renaming `select` would duplicate the `index` already passed.
    code = fields.Char(select=True, index=True)
