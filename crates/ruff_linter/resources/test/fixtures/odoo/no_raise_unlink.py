from odoo import models
from odoo.addons.component.core import Component


class MyModel(models.Model):
    _inherit = "my.model"

    def unlink(self):
        if self.state == "done":
            raise UserError("Cannot delete a done record")
        return super().unlink()

    def write(self, vals):
        if not vals:
            raise UserError("Nothing to write")
        return super().write(vals)


class Named(models.Model):
    _name = "my.other.model"

    def unlink(self):
        raise UserError("Cannot delete")


# A model base without `_name` or `_inherit` declares no model: nothing to report.
class NotAModel(models.Model):
    def unlink(self):
        raise UserError("Cannot delete")


# OCA components reuse `_inherit` for component names, not for ORM records.
class MyComponent(Component):
    _inherit = "base.component"

    def unlink(self):
        raise UserError("Cannot delete")


# A plain Python class that happens to define `unlink`.
class Helper:
    _inherit = "not.odoo"

    def unlink(self):
        raise ValueError("nope")


# `CachedModel` is a model base too, added in Odoo 20.0 for the registry-cached models.
class Company(models.CachedModel):
    _inherit = "res.company"

    def unlink(self):
        raise UserError("Cannot delete")


# `BaseModel` is what a report over a SQL view subclasses.
class SkillReport(models.BaseModel):
    _name = "hr.employee.skill.report"
    _auto = False

    def unlink(self):
        raise UserError("Cannot delete")


# `Constraint` ships from the same module and is a table object, not a model.
class VirtualConstraint(models.Constraint):
    def unlink(self):
        raise UserError("Cannot delete")
