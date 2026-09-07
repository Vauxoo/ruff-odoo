"""Loading every record of a model is what a test does on purpose.

Dropping the enclosing-model-class requirement, which is what lets the rule reach a
controller, would otherwise reach here too: `self.env[...]` resolves the model just as well
inside a `TransactionCase`. `tests/` is a directory name Odoo requires, so this file is
recognised as one Odoo runs outside the request cycle and left alone.
"""

from odoo.tests.common import TransactionCase


class TestSearch(TransactionCase):
    def test_every_partner(self):
        # Not reported: a test loading the whole table is deliberate.
        return self.env["res.partner"].search([])

    def test_deprecated_call(self):
        # Not reported either, for the same reason.
        return self.env["res.partner"].check_access_rights("read")
