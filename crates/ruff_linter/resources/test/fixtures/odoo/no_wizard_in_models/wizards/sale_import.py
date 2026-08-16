from odoo import models


class SaleImport(models.TransientModel):
    # Not flagged: wizards/ is the right directory.
    _name = "sale.import"
