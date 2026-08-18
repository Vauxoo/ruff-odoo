"""Aliased to another name: the import is de-aliased and every use renamed."""

from odoo.exceptions import Warning as OdooWarning


def check(record):
    if not record.active:
        raise OdooWarning("Inactive record")
