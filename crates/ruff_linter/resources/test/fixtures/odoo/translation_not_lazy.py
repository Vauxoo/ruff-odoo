from odoo import _
from odoo.tools.translate import _lt


class Model:
    def method(self, var, name, count, total):
        # Interpolation applied to the translated result.
        _("Hello %s") % var  # ODW8301 (fixable)
        _("%s of %s") % (count, total)  # ODW8301 (fixable)
        _("Hello %(name)s") % {"name": name}  # ODW8301 (fixable)
        self.env._("Hello %s") % var  # ODW8301 (fixable)
        self.env._("Hello %s") % (var,)  # ODW8301 (fixable)
        _(name) % (count, total)  # ODW8301 (fixable: tuple converts for any term)

        # Interpolation applied inside the term.
        _("Hello %s" % var)  # ODW8301 (fixable)
        _("Hello %(name)s" % {"name": name})  # ODW8301 (fixable)
        self.env._("Hello %s" % var)  # ODW8301 (fixable)
        _(name % var)  # ODW8301

        # Concatenation gluing a literal to a value.
        _("Hello " + var)  # ODW8301
        _(var + " Hello")  # ODW8301
        _("Hello " + "dear " + var)  # ODW8301

        # Diagnostics without a fix.
        _("Hello %(name)s") % name  # single value against a named placeholder
        _("Hello") % var  # no placeholder consumes the value
        _("Hello %(class)s") % {"class": name}  # key is a Python keyword
        _("Hello %(source)s") % {"source": name}  # key taken by the signature
        _("Hello %s") % (  # comment anchored inside the expression
            var,
        )

        # No diagnostic: literal-only concatenation is one constant term.
        _("Hello " + "world")
        # No diagnostic: the values are already translation arguments.
        _("Hello %s", var)
        # No diagnostic: `_lt` lazy translations are excluded.
        _lt("Hello %s") % var
        _lt("Hello %s" % var)
        # No diagnostic: not a translation function.
        "Hello %s" % var
        # No diagnostic: the term is not the lone argument.
        _("Hello %s", var) % var
