from odoo import fields, models


class MyModel(models.Model):
    _inherit = "my.model"

    name = fields.Char("Name")

    user_id = fields.Many2one("res.users", string="User")

    state = fields.Selection([("a", "A")], "State")

    other = fields.Char(string="Different Label")

    name33 = fields.Char("Name33", related="partner_id.name")

    def my_method(self):
        name = fields.Char("Name")


class OrdinaryPythonClass:
    name = fields.Char(string="Name")


class OtherModel(models.Model):
    _inherit = "other.model"

    partner_id = fields.Many2one("res.partner")
