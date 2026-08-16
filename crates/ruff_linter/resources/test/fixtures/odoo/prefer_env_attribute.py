from odoo import models
from odoo.http import request


class MyModel(models.Model):
    _inherit = "my.model"

    def action_do(self):
        self._cr.execute("SELECT 1")
        self._uid
        self._context.get("lang")
        self.env.cr.execute("SELECT 2")
        self.env.uid
        self.env.context.get("lang")


class MyController:
    def handle(self):
        request._cr.execute("SELECT 1")
        request._uid
        request._context.get("lang")
        request.env.context.get("lang")


class OrdinaryPythonClass:
    def action_do(self):
        self._cr.execute("SELECT 1")
        self._context.get("lang")


class Model:
    """A class that just happens to be named like an Odoo base, from elsewhere."""


class NotOdooModel(Model):
    def action_do(self):
        self._cr.execute("SELECT 1")


def shadowed_request(request):
    """A parameter named `request` shadows the import, so it isn't Odoo's."""
    return request._context
