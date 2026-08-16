class MyModel(models.Model):
    _inherit = "my.model"

    def write(self, vals):
        return True

    def write(self, vals):
        return super().write(vals)

    def create(self, vals):
        if vals:
            return super().create(vals)
        return {}

    def action_confirm(self):
        return True

    @api.model
    def default_get(self, fields_list):
        res = super().default_get(fields_list)
        res.update({})
        return res


def write(vals):
    return True
