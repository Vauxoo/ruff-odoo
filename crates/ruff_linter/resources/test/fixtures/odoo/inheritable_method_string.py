class MyModel(models.Model):
    _inherit = "my.model"

    total = fields.Float(compute=_compute_total)
    other = fields.Float(compute="_compute_other")
    has_google_tagmanager = fields.Boolean(inverse=inverse_has_google_tagmanager)

    def _compute_total(self):
        pass

    def _compute_other(self):
        pass

    def inverse_has_google_tagmanager(self):
        pass


class NotDefinedHere(models.Model):
    _inherit = "my.model"

    # No diagnostic (and no fix): `_compute_external` is not a method of this class.
    external = fields.Float(compute=_compute_external)
