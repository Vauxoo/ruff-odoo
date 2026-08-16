from odoo import fields, models


class SaleImport(models.TransientModel):
    # Flagged: a wizard defined in the models/ directory.
    _name = "sale.import"


class ResConfigSettings(models.TransientModel):
    # Not flagged: settings screens conventionally live in models/.
    _inherit = "res.config.settings"

    group_discount = fields.Boolean()


class SaleOrder(models.Model):
    # Not flagged: regular model.
    _inherit = "sale.order"
