"""The class is an Odoo model but `fields` is not Odoo's, so its arguments are not the ones
this rule knows: removing the string would be removing a real argument."""

from odoo import models

from . import fields


class MyModel(models.Model):
    _inherit = "my.model"

    name = fields.Char("Name")
