class MyModel(models.Model):
    _inherit = "my.model"

    def name_get(self):
        return []

    def _compute_display_name(self):
        pass
