from other_library import ustr, to_text
from odoo.tools import mute_logger


def run(value):
    return ustr(value), to_text(value)
