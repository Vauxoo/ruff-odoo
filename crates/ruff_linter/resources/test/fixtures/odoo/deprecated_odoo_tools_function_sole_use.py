from odoo.tools import ustr


def label(word):
    return ustr(word, errors="replace")
