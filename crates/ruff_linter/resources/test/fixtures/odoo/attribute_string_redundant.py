from odoo import fields, models


class MyModel(models.Model):
    _inherit = "my.model"

    name = fields.Char("Name")

    user_id = fields.Many2one("res.users", string="User")

    state = fields.Selection([("a", "A")], "State")

    partner_ids = fields.Many2many(
        "res.partner", "table_name", "col1", "col2", "Partner"
    )

    other = fields.Char(string="Different Label")

    name33 = fields.Char("Name33", related="partner_id.name")

    def my_method(self):
        name = fields.Char("Name")


class OrdinaryPythonClass:
    name = fields.Char(string="Name")


class OtherModel(models.Model):
    _inherit = "other.model"

    partner_id = fields.Many2one("res.partner")


class MoreFieldTypes(models.Model):
    _inherit = "more.model"

    description = fields.Text(string="Description")

    create_date = fields.Datetime("Create Date")

    amount = fields.Monetary(string="Amount")

    res_id = fields.Reference([], "Res")

    line_ids = fields.One2many("my.line", "parent_id", "Line")
