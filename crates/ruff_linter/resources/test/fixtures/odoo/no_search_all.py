from odoo import models


class MyModel(models.Model):
    _inherit = "my.model"

    def action_all(self):
        all_partners = self.env["res.partner"].search([])
        limited = self.env["res.partner"].search([], limit=100)
        filtered = self.env["res.partner"].search([("active", "=", True)])
        counted = self.env["res.partner"].search([], count=True)
        read_all = self.env["res.partner"].search_read([])
        return all_partners, limited, filtered, counted, read_all

    def action_domain_variable(self):
        domain = []
        all_partners = self.env["res.partner"].search(domain)

        built_domain = []
        built_domain.append(("active", "=", True))
        built_partners = self.env["res.partner"].search(built_domain)
        return all_partners, built_partners
