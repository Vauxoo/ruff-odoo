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


class EveryFieldShape(models.Model):
    _inherit = "every.model"

    # Position 0: the string is the first positional argument, with or without company.
    name = fields.Char(string="Name", required=True)

    active = fields.Boolean("Active", default=True)

    age = fields.Integer(string="Age")

    score = fields.Float("Score")

    body = fields.Html(string="Body")

    date_order = fields.Date(string="Date Order")

    file_data = fields.Binary(string="File Data")

    # Position 1: Selection, Reference and Many2one take the string after their first argument.
    type = fields.Selection(selection=[], string="Type")

    partner_id = fields.Many2one("res.partner", "Partner")

    # Position 2: One2many takes it after the comodel and the inverse name.
    child_ids = fields.One2many("my.child", "parent_id", string="Child")

    # No diagnostic: Many2many is `(comodel_name, relation, column1, column2, string)`, so the
    # second positional is the relation table, not a label -- removing it would change the
    # schema. Only the fifth positional is the string, as `partner_ids` above shows.
    tag_ids = fields.Many2many("res.tag", "Tag")
