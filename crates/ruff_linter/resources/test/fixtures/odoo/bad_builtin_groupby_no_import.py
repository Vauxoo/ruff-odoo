"""`odoo.tools.groupby` is not imported yet: the fix adds the import."""

import itertools

itertools.groupby(records, key=lambda r: r.partner_id)
