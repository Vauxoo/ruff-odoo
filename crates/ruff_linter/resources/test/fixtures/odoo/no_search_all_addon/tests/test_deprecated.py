"""The same reasoning for `deprecated-odoo-method-call`.

Its receiver test admits an `env[...]` subscript outside a model class, which is what lets
it reach a controller. A test file resolves the model the same way, so it needs the same
exclusion.
"""

from odoo.tests.common import TransactionCase


class TestDeprecated(TransactionCase):
    def test_deprecated_call(self):
        # Not reported: Odoo runs this outside the request cycle.
        return self.env["res.partner"].check_access_rights("read")
