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

    def action_model_not_listed(self):
        # These models stay small, so an unlimited search on them is not worth reporting.
        users = self.env["res.users"].search([])
        params = self.env["ir.config_parameter"].search([])
        companies = self.env["res.company"].search_read([])
        return users, params, companies

    def action_glob_match(self):
        # "account.move*" covers the lines too.
        moves = self.env["account.move"].search([])
        lines = self.env["account.move.line"].search([])
        return moves, lines

    def action_model_through_variable(self):
        orders = self.env["sale.order"]
        all_orders = orders.search([])

        settings = self.env["res.config.settings"]
        all_settings = settings.search([])
        return all_orders, all_settings

    def action_passthrough_chain(self):
        moves = self.env["stock.move"].sudo().search([])
        quants = self.env["stock.quant"].with_context(active_test=False).search([])
        return moves, quants

    def action_model_unresolved(self, model_name):
        # The model is only known at runtime, so the call is left alone.
        records = self.env[model_name].search([])
        other = self.env[self.some_field].search([])
        return records, other


class AccountMove(models.Model):
    _name = "account.move"

    def action_self_search(self):
        # `self` runs against the model the class declares.
        return self.search([])


class ResConfigSettings(models.TransientModel):
    _name = "res.config.settings"

    def action_self_search(self):
        # Same shape, but this model is not one that grows.
        return self.search([])


class MultiInherit(models.Model):
    _inherit = ["mail.thread", "account.move"]

    def action_self_search(self):
        # A multi-model `_inherit` names no single model, so nothing is reported.
        return self.search([])
