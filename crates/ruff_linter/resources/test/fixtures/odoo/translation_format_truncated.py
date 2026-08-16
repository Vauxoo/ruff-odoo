from odoo import _


class Model:
    def method(self, var):
        _("Hello %", var)  # ODE8301
        self.env._("Hello %", var)  # ODE8301
        _("%s %", var)  # ODE8301
        _("100%", var)  # ODE8301

        # No diagnostic: without supplied values the term is used verbatim.
        _("Hello %")
        _("100%")
        # No diagnostic: complete conversion specifiers.
        _("Hello %s", var)
        _("Hello %%", "100")
