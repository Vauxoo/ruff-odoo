from odoo import _


class Model:
    def method(self, var):
        _("Hello %y", var)  # ODOO062
        self.env._("Hello %y", var)  # ODOO062

        # No diagnostic: without supplied values the term is used verbatim.
        _("Hello %y")
        # No diagnostic: supported conversions, flags, width and precision.
        _("Hello %s", var)
        _("Hello %05.2f", var)
        _("Hello %(name)s", var)
