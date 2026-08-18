"""A `_` that is not Odoo's: reported, the way pylint-odoo reported the name, but never
rewritten -- `self.env._` is no replacement for a function that came from somewhere else."""

from gettext import gettext as _

from odoo import models


class MyModel(models.Model):
    _inherit = "my.model"

    def gettext_translation(self):
        return _("from the standard library")

    def locally_shadowed(self):
        _ = lambda *args: True  # noqa: E731
        return _("a local of its own")
