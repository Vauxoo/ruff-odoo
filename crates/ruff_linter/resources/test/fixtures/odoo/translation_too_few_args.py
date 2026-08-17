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

        # No diagnostic: the term is interpolated after the call.
        raise UserError(_("%s of %s") % (count, name))

    def raising(self, name, count):
        # The lone argument of a raised exception can not be interpolated later, so the
        # placeholders left in the term are never filled.
        raise UserError(_("%s of %s"))  # ODE8306

    def raising_odoo_exception(self, name):
        raise ValidationError(_("record <%s: (%s)> is not valid"))  # ODE8306

    def raising_ok(self, name, count):
        # No diagnostic: the translation call fills the placeholders itself.
        raise UserError(_("%s of %s", count, name))

    def raising_prose(self):
        # No diagnostic: with nothing to interpolate, a `%` in prose is not a conversion
        # worth reporting -- "% o" here is a space-flagged `%o`, and "50%" a truncated one.
        raise UserError(_("100% off"))

    def raising_truncated(self):
        raise UserError(_("discount of 50%"))

    def raising_mixed(self, name):
        # ODE8306: the space-flagged conversion is prose, but the `%s` is unmistakable.
        raise UserError(_("%s got 100% off"))

    def raising_not_alone(self, name):
        # No diagnostic: the exception takes more than the term, out of scope.
        raise UserError(_("%s of %s"), name)
