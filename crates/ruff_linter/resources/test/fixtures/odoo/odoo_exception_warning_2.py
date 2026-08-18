"""Aliased as `UserError` already: dropping the alias is the whole fix."""

from odoo.exceptions import Warning as UserError


def check(record):
    if not record.active:
        raise UserError("Inactive record")
