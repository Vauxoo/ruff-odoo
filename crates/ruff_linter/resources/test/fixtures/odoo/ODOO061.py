from odoo import _


class Model:
    def method(self, var, name):
        _("Hello %s", var, name)  # ODOO061
        self.env._("Hello", var)  # ODOO061
        _("Hello %%", var)  # ODOO061 (`%%` does not consume a value)

        # No diagnostic: counts match.
        _("Hello %s", var)
        # No diagnostic: keyword arguments are not counted.
        _("Hello %s", var, other=name)
        # No diagnostic: star arguments are out of scope.
        _("Hello %s", *var)
