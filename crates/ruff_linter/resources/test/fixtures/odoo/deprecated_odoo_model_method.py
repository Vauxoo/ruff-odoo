from odoo import models


class MyModel(models.Model):
    _inherit = "my.model"

    def fields_view_get(self, view_id=None, view_type="form", **kwargs):
        return {}

    def fields_get(self):
        return {}

    def check_access_rights(self, operation, raise_exception=True):
        """Odoo 18.0 folded the access hooks into `_check_access`."""
        return super().check_access_rights(operation, raise_exception)

    def check_access_rule(self, operation):
        return super().check_access_rule(operation)

    def _filter_access_rules(self, operation):
        return super()._filter_access_rules(operation)

    def _filter_access_rules_python(self, operation):
        return super()._filter_access_rules_python(operation)

    def _check_access(self, operation):
        """The replacement hook itself is not deprecated."""
        return super()._check_access(operation)

    def check_access(self, operation):
        return super().check_access(operation)


class OrdinaryPythonClass:
    """Not an Odoo model, so an identically named method is none of the rule's business."""

    def check_access_rights(self, operation, raise_exception=True):
        return True
