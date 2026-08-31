from odoo.fields import Domain
from odoo.osv import expression

# The fix reuses the `Domain` already in scope instead of importing it again.
expression.AND([domain, [("state", "=", "draft")]])
Domain.AND([domain, [("state", "=", "draft")]])
