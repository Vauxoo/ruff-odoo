from odoo import _
from odoo.tools.translate import _lt


class Model:
    def method(self, var):
        _("Hello {}").format(var)  # ODW8302
        self.env._("Hello {}").format(var)  # ODW8302
        _("Hello {}".format(var))  # ODW8302
        self.env._("Hello {x}".format(x=var))  # ODW8302
        _("Hello {} and {}").format(var, var)  # ODW8302
        _("{{literal}} 100% {}".format(var))  # ODW8302
        # Flagged, but no fix: an escape sequence could hide a brace or `%`.
        _("Hello {}\n".format(var))  # ODW8302

        # No diagnostic: a non-empty format spec is too complex to convert.
        _("Hello {:>10}").format(var)
        _("Hello {:>10}".format(var))
        # No diagnostic: the term is not a literal.
        _(var).format(var)
        # No diagnostic: extra translation arguments take over the interpolation.
        _("Hello {}", var).format(var)
        # No diagnostic: not a translation function.
        "Hello {}".format(var)
        # No diagnostic: `_lt` lazy translations are excluded.
        _lt("Hello {}").format(var)
        _lt("Hello {}".format(var))
