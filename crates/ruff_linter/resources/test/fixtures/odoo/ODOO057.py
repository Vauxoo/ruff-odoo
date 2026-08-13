from odoo import _


class Model:
    def method(self, var):
        _("Hello %", var)  # ODOO057
        self.env._("Hello %", var)  # ODOO057
        _("%s %", var)  # ODOO057
        _("100%", var)  # ODOO057

        # No diagnostic: without supplied values the term is used verbatim.
        _("Hello %")
        _("100%")
        # No diagnostic: complete conversion specifiers.
        _("Hello %s", var)
        _("Hello %%", "100")
