import logging

from odoo import models

_logger = logging.getLogger(__name__)


class Test(models.Model):
    _name = "x.test"
