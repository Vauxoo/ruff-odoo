from odoo import _
from odoo.tools.translate import _lt


class Model:
    def method(self, var):
        _(f"Hello {var}")  # ODW8303
        self.env._(f"Hello {var}")  # ODW8303
        _(f"Hello")  # ODW8303
        _(f"Hello {var}" f" and {var}")  # ODW8303

        # No diagnostic: the `%` placeholders show the interpolation is printf's.
        _(f"Hello %s {var}")
        # No diagnostic: plain strings.
        _("Hello")
        _("Hello %s", var)
        # No diagnostic: `_lt` lazy translations are excluded.
        _lt(f"Hello {var}")
