from odoo import fields, models


class SaleCommissionPlanUserWizard(models.TransientModel):
    _name = "sale.commission.plan.user.wizard"

    # The label lands where the relation table goes: capitals give it away.
    user_ids = fields.Many2many("res.users", "Salespersons", domain="[('share', '=', False)]")

    # Same, with a value that could not be a table name at all.
    finished_lot_ids = fields.Many2many("stock.lot", "Finished Lot/Serial", related="production_id.lot_producing_ids")

    # Lowercase, but the spaces still make it a label rather than an identifier.
    tag_ids = fields.Many2many("res.tag", "some tags")

    # No diagnostic: it reads as a table name, which is what the parameter is for.
    partner_ids = fields.Many2many("res.partner", "wizard_res_partner_rel", "wizard_id", "res_partner_id", "Partners")

    line_ids = fields.Many2many("my.line", "wizard_my_line_rel")

    # No diagnostic: the label is where it belongs.
    user_id_labels = fields.Many2many("res.users", string="Salespersons")

    # No diagnostic: nothing in the relation position.
    company_ids = fields.Many2many("res.company")


class NotAnOdooModel:
    # No diagnostic: the class is not an Odoo model, so this is not an Odoo field.
    user_ids = fields.Many2many("res.users", "Salespersons")
