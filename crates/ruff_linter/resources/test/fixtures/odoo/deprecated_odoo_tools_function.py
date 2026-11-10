import odoo.loglevels
import odoo.tools.misc as misc
from odoo import tools
from odoo.loglevels import get_encodings
from odoo.tools import pycompat, ustr
from odoo.tools.misc import ustr as misc_ustr
from odoo.tools.pycompat import csv_reader, csv_writer, to_text


# Reported, with the safe fix: the argument is a str by construction.
a = ustr("literal")
b = ustr(f"fstring {a}")
c = ustr(str(a))
d = ustr("literal", "latin-1", "replace")

# Reported, no fix: the options are not constants, so dropping them would drop an effect.
e = ustr("literal", errors=handler())

# Reported, with the safe fix: ustr delegates an exception to exception_to_unicode.
try:
    pass
except ValueError as error:
    f = ustr(error)

# Reported, with the unsafe fix to str(): the type of the value is unknown.
g = ustr(value)
h = ustr(value=value)
i = ustr(self.name)
j = tools.ustr(value)
k = misc.ustr(value)
l = odoo.loglevels.ustr(value)
m = misc_ustr(value)

# Reported, with the unsafe fix to .decode(): the options only make sense for bytes.
n = ustr(word, errors="replace")
o = ustr(word, "latin-1")
p = ustr(word, "latin-1", "ignore")
q = ustr(word, hint_encoding="latin-1", errors="replace")
r = ustr(first + second, errors="replace")
s = ustr(b"raw", errors="replace")

# Reported, no fix.
t = ustr(*args)
u = ustr(value, bogus=True)
v = map(ustr, values)
w = list(get_encodings("utf-8"))

# pycompat.
x = to_text("literal")
y = to_text(value)
z = pycompat.to_text(None)
reader = csv_reader(stream)
reader = csv_reader(stream, delimiter=";")
writer = csv_writer(stream, quoting=1)
reader = pycompat.csv_reader(stream)
reader = csv_reader(stream=stream)


# Not reported: names that are not the Odoo helpers.
def local():
    def ustr(value):
        return value

    return ustr("x")


def csv_reader(stream):
    return stream


class Other:
    def to_text(self, value):
        return value

    def run(self, value):
        return self.to_text(value)


def uses_builtins(value):
    return str(value), value.decode(), csv_reader(value)


csv_writer = None
