from odoo import models


class MyModel(models.Model):
    _inherit = "my.model"

    def action_do(self):
        self._cr.execute("SELECT 1")
        self.env.cr.execute("SELECT 2")


class OrdinaryPythonClass:
    def action_do(self):
        self._cr.execute("SELECT 1")


class Model:
    """A class that just happens to be named like an Odoo base, from elsewhere."""


class NotOdooModel(Model):
    def action_do(self):
        self._cr.execute("SELECT 1")
