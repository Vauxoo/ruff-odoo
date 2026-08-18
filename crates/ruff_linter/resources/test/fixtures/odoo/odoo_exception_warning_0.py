"""`UserError` is free: the import is retargeted and every use renamed."""

from odoo.exceptions import Warning

__all__ = ["Warning"]


def check(record):
    if not record.active:
        raise Warning("Inactive record")
