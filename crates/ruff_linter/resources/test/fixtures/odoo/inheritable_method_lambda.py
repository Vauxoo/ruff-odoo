class MyModel(models.Model):
    _inherit = "my.model"

    company_id = fields.Many2one("res.company", default=_default_company)
    user_id = fields.Many2one("res.users", default=lambda self: self.env.user)
    partner_id = fields.Many2one("res.partner", domain=_domain_partner)

    def _default_company(self):
        return self.env.company

    def _domain_partner(self):
        return [("active", "=", True)]


class NotDefinedHere(models.Model):
    _inherit = "my.model"

    # No diagnostic (and no fix): `_default_currency` is not a method of this class.
    currency_id = fields.Many2one("res.currency", default=_default_currency)
