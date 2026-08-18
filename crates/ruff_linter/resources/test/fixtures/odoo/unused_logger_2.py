import logging

from odoo import models

# `getLogger` with anything other than `__name__` is somebody else's logger: it may be
# grabbed by name from elsewhere, so an unused-looking assignment here is not dead.
_logger = logging.getLogger("other name")


class Test(models.Model):
    _name = "x.test"
