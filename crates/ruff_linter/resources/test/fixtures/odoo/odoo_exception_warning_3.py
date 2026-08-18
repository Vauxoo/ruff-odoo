"""`UserError` is bound to something else: flagged, but no fix is offered."""

from odoo.exceptions import Warning


class UserError(Exception):
    pass


def check(record):
    if not record.active:
        raise Warning("Inactive record")
