"""`UserError` is already imported: `Warning` is dropped in its favor."""

from odoo.exceptions import UserError, Warning


def check(record):
    if not record.active:
        raise Warning("Inactive record")
    if not record.name:
        raise UserError("Missing name")
