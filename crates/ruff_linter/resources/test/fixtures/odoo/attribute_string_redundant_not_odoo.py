"""Neither the class nor the field descriptors are Odoo's here, so nothing is reported:
the label this rule reasons about is the one Odoo infers, and only Odoo infers it."""

from odoo import fields

from . import models as models


class NotAnOdooModel(models.Model):
    # `models` is not `odoo.models`, so the base does not make this an Odoo model.
    name = fields.Char("Name")
