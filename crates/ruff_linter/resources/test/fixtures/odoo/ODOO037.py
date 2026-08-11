class MyModel(models.Model):
    _inherit = "my.model"

    def fields_view_get(self, view_id=None, view_type="form", **kwargs):
        return {}

    def fields_get(self):
        return {}
