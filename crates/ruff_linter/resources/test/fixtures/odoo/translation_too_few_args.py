from odoo import _


class Model:
    def method(self, name, count):
        _("%s of %s", count)  # ODE8306
        self.env._("%s of %s", count)  # ODE8306
        _("%s: %.*f", name)  # ODE8306 (`.*` consumes an extra value)

        # No diagnostic: counts match.
        _("%s of %s", count, name)
        # No diagnostic: mapping keys pair with keywords, out of scope.
        _("%(count)s of %(total)s", count)
        # No diagnostic: without supplied values the term is used verbatim.
        _("%s of %s")
