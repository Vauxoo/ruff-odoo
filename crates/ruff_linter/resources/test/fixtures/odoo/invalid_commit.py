class MyModel(models.Model):
    _inherit = "my.model"

    def write(self, vals):
        self.env.cr.commit()
        return super().write(vals)

    def unlink(self):
        self.env.cr.execute("SELECT 1")
        return super().unlink()


def post_init_hook(env):
    # Flagged: `env.cr` is the cursor of the environment Odoo hands to the hook.
    env.cr.commit()
