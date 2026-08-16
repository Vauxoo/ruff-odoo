class MyModel(models.Model):
    _inherit = "my.model"

    def write(self, vals):
        super().write(vals)

    def unlink(self):
        return super().unlink()

    def create(self, vals):
        if vals:
            return super().create(vals)
        return {}

    def action_do(self):
        super().action_do()
