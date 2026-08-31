from odoo.models import Model


class ResCurrency(Model):
    _name = "res.currency"

    # ODE9501: `models` is not bound here, so the fix has to import it as well.
    _sql_constraints = [
        ("name_uniq", "unique (name)", "The currency name must be unique!"),
    ]
