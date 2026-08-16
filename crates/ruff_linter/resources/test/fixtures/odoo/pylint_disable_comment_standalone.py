"""Standalone `# pylint: disable` pragmas, checked with their target rules enabled.

Unlike `ODC8502.py`, this fixture is linted with the rules the pragmas name, so the
block-scoped pragmas can be rewritten into the `# noqa` comments that replace them.
"""

from odoo import models


class SaleOrder(models.Model):
    _inherit = "sale.order"

    def unlink(self):
        # pylint: disable=method-required-super
        # The pragma is the first line of the body, so it also covers the `def` header,
        # which is where `method-required-super` is reported.
        return True

    def create_address(self, values):
        # pylint: disable=context-overridden
        context = {"lang": "en_US"}
        return self.env["res.partner"].with_context(context).create(values)

    def obsolete(self):
        # pylint: disable=invalid-commit
        # Nothing in this block commits, so the pragma only has to be removed.
        return True

    def already_suppressed(self):
        # pylint: disable=invalid-commit
        self.env.cr.commit()  # noqa: ODE8102

    def partially_mapped(self):
        # pylint: disable=invalid-commit,some-custom-check
        self.env.cr.commit()

    def disable_next(self):
        # pylint: disable-next=invalid-commit
        self.env.cr.commit()
        self.env.cr.commit()


# A module-level pragma runs to the end of the file in pylint, even though it reads as if
# it only applied to the function below it — so the function after it is covered too.
# pylint: disable=invalid-commit
def module_level(cr):
    cr.commit()


def also_covered(cr):
    cr.commit()
