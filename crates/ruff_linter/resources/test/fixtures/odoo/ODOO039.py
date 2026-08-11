class MyModel(models.Model):
    _inherit = "my.model"

    def unlink(self):
        if self.state == "done":
            raise UserError("Cannot delete a done record")
        return super().unlink()

    def write(self, vals):
        if not vals:
            raise UserError("Nothing to write")
        return super().write(vals)
