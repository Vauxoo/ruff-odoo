class MyModel(models.Model):
    _inherit = "my.model"

    company_id = fields.Many2one("res.company", default=_default_company)
    user_id = fields.Many2one("res.users", default=lambda self: self.env.user)

    def _default_company(self):
        return self.env.company
