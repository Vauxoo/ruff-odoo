"""`_lt` bound to Odoo's `LazyTranslate`, the way Odoo 18.0 and later declare it. Its calls
are not reported, anywhere in the file, while the bare `_` next to them still is."""

from odoo import _, models
from odoo.tools import LazyTranslate

_lt = LazyTranslate(__name__)

LABELS = {"greeting": _lt("Hello")}


class MyModel(models.Model):
    _inherit = "my.model"

    def method(self):
        lazy = _lt("Deferred")
        eager = _("Reported")
        return lazy, eager
