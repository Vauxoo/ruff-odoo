from odoo import _, api, fields, models


class ResPartnerCategory(models.Model):
    _name = "res.partner.category"

    name = fields.Char()

    # ODE9501: the common shape, one entry with a key, a definition and a message.
    _sql_constraints = [
        ("name_uniq", "unique (name)", "The name must be unique!"),
    ]


class AccountAnalyticLine(models.TransientModel):
    _name = "account.analytic.line"

    # ODE9501: several entries, and one of them with no message at all.
    _sql_constraints = [
        ("check_amount", "CHECK (amount > 0)", "The amount must be positive."),
        ("account_uniq", "unique (account_id, date)"),
    ]


class SaleOrderLine(models.Model):
    _name = "sale.order.line"

    # ODE9501: written on a single line, and with a translated message.
    _sql_constraints = [("qty_uniq", "unique (qty)", _("Quantity must be unique."))]


class StockQuant(models.Model):
    _name = "stock.quant"

    # ODE9501: a message too long to keep the call on one line once rewritten, so the fix
    # expands it with one argument per line and a magic trailing comma.
    _sql_constraints = [
        (
            "product_location_uniq",
            "unique (product_id, location_id, lot_id, package_id, owner_id)",
            "A quant can only exist once per product, location, lot, package and owner.",
        ),
    ]


class ProjectTask(models.AbstractModel):
    _name = "project.task"

    # ODE9501, but no fix: the value is not a list of tuples this rule can take apart.
    _sql_constraints = BASE_SQL_CONSTRAINTS


class ProjectMilestone(models.Model):
    _name = "project.milestone"

    # ODE9501, but no fix: extending another model's list is not a literal either.
    _sql_constraints = ProjectTask._sql_constraints + [
        ("name_uniq", "unique (name)", "The name must be unique!"),
    ]


class MrpProduction(models.Model):
    _name = "mrp.production"

    # ODE9501, but no fix: `_name_uniq` would become `__name_uniq`, which Python mangles.
    _sql_constraints = [
        ("_name_uniq", "unique (name)", "The name must be unique!"),
    ]


class MrpWorkorder(models.Model):
    _name = "mrp.workorder"

    # ODE9501, but no fix: the key cannot be spelled as an attribute.
    _sql_constraints = [
        ("name uniq", "unique (name)", "The name must be unique!"),
    ]


class HrEmployee(models.Model):
    _name = "hr.employee"

    _barcode_uniq = fields.Char()

    # ODE9501, but no fix: the class already binds `_barcode_uniq`.
    _sql_constraints = [
        ("barcode_uniq", "unique (barcode)", "The barcode must be unique!"),
    ]


class HrDepartment(models.Model):
    _name = "hr.department"

    # ODE9501, but no fix: both entries would land on `_name_uniq`.
    _sql_constraints = [
        ("name_uniq", "unique (name)", "The name must be unique!"),
        ("name_uniq", "unique (complete_name)", "The complete name must be unique!"),
    ]


class CrmLead(models.Model):
    _name = "crm.lead"

    # ODE9501, but no fix: the rewrite would drop the comment inside the list.
    _sql_constraints = [
        # Only enforced on records created after the migration.
        ("name_uniq", "unique (name)", "The name must be unique!"),
    ]


class CrmStage(models.Model):
    _name = "crm.stage"

    # ODE9501, but no fix: the message is spread over two lines.
    _sql_constraints = [
        (
            "name_uniq",
            "unique (name)",
            "The name must be unique, because the stage is looked up by name "
            "in every pipeline report.",
        ),
    ]


class CrmTeam(models.Model):
    _name = "crm.team"

    # ODE9501, but no fix: an empty list has no attribute to be rewritten into.
    _sql_constraints = []


class CrmTeamMember(models.Model):
    _name = "crm.team.member"

    # ODE9501, but no fix: four elements is not a shape `models.Constraint` accepts.
    _sql_constraints = [
        ("name_uniq", "unique (name)", "The name must be unique!", "extra"),
    ]


class ResUsers(models.Model):
    _name = "res.users"

    @api.model
    def _register_hook(self):
        # Not reported: a local, not a class attribute.
        _sql_constraints = [("login_uniq", "unique (login)", "Login must be unique!")]
        return _sql_constraints


class NotAnOdooModel:
    # Not reported: the class is not an Odoo model.
    _sql_constraints = [
        ("name_uniq", "unique (name)", "The name must be unique!"),
    ]
