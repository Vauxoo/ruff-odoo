from odoo import _
from odoo.tools.translate import _lt


class Model:
    def method(self, var):
        _(f"Hello {var}")  # ODOO058
        self.env._(f"Hello {var}")  # ODOO058
        _(f"Hello")  # ODOO058
        _(f"Hello {var}" f" and {var}")  # ODOO058

        # No diagnostic: the `%` placeholders show the interpolation is printf's.
        _(f"Hello %s {var}")
        # No diagnostic: plain strings.
        _("Hello")
        _("Hello %s", var)
        # No diagnostic: `_lt` lazy translations are excluded.
        _lt(f"Hello {var}")
