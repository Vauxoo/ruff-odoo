import itertools

from odoo.tools import groupby

itertools.groupby(records, key=lambda r: r.partner_id)
groupby(records, key=lambda r: r.partner_id)
