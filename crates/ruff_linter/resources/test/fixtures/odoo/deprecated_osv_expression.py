import odoo.osv.expression
from odoo.osv import expression
from odoo.osv import expression as expr
from odoo.osv.expression import AND, OR, TRUE_DOMAIN, is_leaf, normalize_domain

# Fixable: a pure forwarder to a `Domain` attribute.
expression.AND([domain, [("state", "=", "draft")]])
expression.OR([domain, [("state", "=", "draft")]])
expression.TRUE_DOMAIN
expression.FALSE_DOMAIN

# The same members reached through the other import shapes.
odoo.osv.expression.AND([domain, [("state", "=", "draft")]])
expr.OR([domain, [("state", "=", "draft")]])
AND([domain, [("state", "=", "draft")]])
OR([domain, [("state", "=", "draft")]])
TRUE_DOMAIN

# Reported without a fix: the replacement is not a drop-in.
expression.normalize_domain(domain)
expression.distribute_not(domain)
expression.is_false(self, domain)
expression.domain_combine_anies(domain, self)
expression.combine("&", [], [], domains)
expression.is_leaf(domain[0])
normalize_domain(domain)
is_leaf(domain[0])
expression.TRUE_LEAF
expression.NOT_OPERATOR
expression.NEGATIVE_TERM_OPERATORS


# The module itself is not reported: it is only the vehicle for the members above, and
# reporting it would flag every call twice.
def use_module():
    return expression


# Not `odoo.osv.expression`: a local binding that happens to share the name.
def local_and():
    def AND(domains):
        return domains

    return AND([domain])
