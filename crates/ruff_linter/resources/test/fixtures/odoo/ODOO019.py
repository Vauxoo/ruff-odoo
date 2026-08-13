import itertools
from itertools import groupby as itertools_groupby

from odoo.tools import groupby

itertools.groupby(records, key=lambda r: r.partner_id)
itertools_groupby(records, key=lambda r: r.partner_id)
groupby(records, key=lambda r: r.partner_id)
