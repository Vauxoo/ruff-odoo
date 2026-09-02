from odoo import fields, models
from odoo import models as odoo_models
from odoo.fields import Char


class ResPartner(models.Model):
    _inherit = "res.partner"

    # OD: the two declarations name different comodels and different relation tables; only
    # the last one is created, so the first names a table that never exists.
    category_ids = fields.Many2many(
        "res.partner.category.report",
        relation="res_partner_res_partner_category_report_rel",
        string="Tags",
    )
    comment = fields.Text()
    category_ids = fields.Many2many(
        "res.partner.category",
        relation="res_partner_res_partner_category_rel",
        string="Tags",
    )


class SaleOrder(models.Model):
    _inherit = "sale.order"

    # OD: byte-for-byte identical declarations, repeated further down the class.
    summary_info = fields.Char(compute="_compute_summary")
    summary_names = fields.Char(compute="_compute_summary")
    has_multiple_lines = fields.Boolean(compute="_compute_summary")

    def _compute_summary(self):
        for order in self:
            order.summary_info = ""

    summary_info = fields.Char(compute="_compute_summary")
    summary_names = fields.Char(compute="_compute_summary")
    has_multiple_lines = fields.Boolean(compute="_compute_summary")


class ThreeTimes(models.Model):
    _name = "three.times"

    # OD: every declaration but the last is dead, and all three are listed under a single
    # diagnostic.
    amount = fields.Float()
    amount = fields.Monetary()
    amount = fields.Integer()


class AbstractDuplicate(models.AbstractModel):
    _name = "abstract.duplicate"

    # OD: an AbstractModel is a model too.
    note = fields.Text()
    note = fields.Html()


class WizardDuplicate(models.TransientModel):
    _name = "wizard.duplicate"

    # OD: so is a TransientModel.
    partner_id = fields.Many2one("res.partner")
    partner_id = fields.Many2one("res.users")


class AliasedBase(odoo_models.Model):
    _name = "aliased.base"

    # OD: the base is the same class, reached through an aliased import.
    note = fields.Text()
    note = fields.Html()


class Annotated(models.Model):
    _name = "annotated"

    # OD: an annotated assignment binds the name just like a plain one.
    amount: float = fields.Float()
    amount: float = fields.Monetary()


class BareAnnotation(models.Model):
    _name = "bare.annotation"

    # OD: a bare annotation binds nothing when the class body runs, so it does not stand
    # between these two declarations.
    note = fields.Text()
    note: str
    note = fields.Html()


class ChainedTargets(models.Model):
    _name = "chained.targets"

    # OD: a chained assignment declares both names, and both are declared again below.
    debit = credit = fields.Monetary()
    debit = credit = fields.Float()


# OK: one declaration per field.
class SaleOrderLine(models.Model):
    _inherit = "sale.order.line"

    summary_info = fields.Char()
    summary_names = fields.Char()


# OK: the same field declared in another class of the same file is Odoo inheritance, not a
# redeclaration, even when both classes extend the same model.
class SaleOrderExtra(models.Model):
    _inherit = "sale.order"

    summary_info = fields.Char()


# OK: not an Odoo model class, so its duplicated attributes are somebody else's business.
class Plain:
    category_ids = fields.Many2many("res.partner.category")
    category_ids = fields.Many2many("res.users")


# OK: a base that resolves to something other than an Odoo model is rejected too.
class PlainWithBase(object):
    category_ids = fields.Many2many("res.partner.category")
    category_ids = fields.Many2many("res.users")


class NotFields(models.Model):
    _name = "not.fields"

    # OK: a duplicated class attribute that is not a field declaration. PIE794 covers those.
    _description = "First"
    _description = "Second"

    # OK: a field declared once, then rebound to something that is not a field call.
    active = fields.Boolean()
    active = None


class RebindWins(models.Model):
    _name = "rebind.wins"

    # OK: the last binding is not a field, so this class declares no `amount` field at all.
    # That is a different problem, and claiming one of these reaches the ORM would be false.
    amount = fields.Float()
    amount = fields.Integer()
    amount = None


class MethodWins(models.Model):
    _name = "method.wins"

    # OK: same shape, with a method taking the name instead of a value.
    quantity = fields.Float()
    quantity = fields.Integer()

    def quantity(self):
        return 0


class ImportedFieldClass(models.Model):
    _name = "imported.field.class"

    # OK (known limitation): a declaration written with the field class imported directly is
    # not recognised as one.
    note = Char()
    note = Char()


class InsideMethod(models.Model):
    _name = "inside.method"

    partner_id = fields.Many2one("res.partner")

    def _prepare(self):
        # OK: a local variable, not a class attribute.
        partner_id = fields.Many2one("res.partner")
        partner_id = fields.Many2one("res.users")
        return partner_id


class NestedClass(models.Model):
    _name = "nested.class"

    partner_id = fields.Many2one("res.partner")

    class Inner:
        # OK: a different class body.
        partner_id = fields.Many2one("res.users")


class ConditionalDeclaration(models.Model):
    _name = "conditional.declaration"

    # OK: only statements directly in the class body are compared, so a declaration guarded
    # by a conditional is left alone.
    partner_id = fields.Many2one("res.partner")

    if hasattr(fields, "Many2oneReference"):
        partner_id = fields.Many2oneReference("res.users")


class TryDeclaration(models.Model):
    _name = "try.declaration"

    # OK: the same, for a declaration inside a `try`.
    partner_id = fields.Many2one("res.partner")

    try:
        partner_id = fields.Many2one("res.users")
    except AttributeError:
        pass


class TupleTargets(models.Model):
    _name = "tuple.targets"

    # OK: unpacking binds each name to a piece of the value, not to a field call.
    first_id, second_id = fields.Many2one("res.partner"), fields.Many2one("res.users")
    first_id, second_id = fields.Many2one("res.users"), fields.Many2one("res.partner")
