import psycopg2
from psycopg2 import sql


class TestModel:
    def injection_binop(self, cr, name, operator):
        # Flagged: values interpolated into the query text.
        cr.execute("SELECT id FROM res_partner WHERE name = '%s'" % name)
        self.env.cr.execute("SELECT id FROM res_partner WHERE name = '%s'" % (name,))
        self._cr.execute("SELECT %s FROM res_partner" % {"column": name})
        cr.execute("SELECT id FROM res_partner WHERE " + operator + " = 1")

    def injection_format_fstring(self, cr, name):
        # Flagged: .format and f-strings with non-constant values.
        cr.execute("SELECT id FROM res_partner WHERE name = '{}'".format(name))
        cr.execute("SELECT id FROM res_partner WHERE name = '{n}'".format(n=name))
        cr.execute(f"SELECT id FROM res_partner WHERE name = '{name}'")

    def injection_variable(self, cr, name):
        # Flagged: the query variable was built by interpolation.
        query = "SELECT id FROM res_partner WHERE name = '%s'" % name
        cr.execute(query)

    def no_injection_parameters(self, cr, name):
        # Not flagged: values passed as query parameters.
        cr.execute("SELECT id FROM res_partner WHERE name = %s", (name,))
        cr.executemany("INSERT INTO res_partner (name) VALUES (%s)", [(name,)])

    def no_injection_private(self, cr):
        # Not flagged: self._table-style private attributes and helper methods.
        cr.execute("SELECT id FROM %s" % self._table)
        cr.execute("SELECT %s FROM %s" % (self._select(), self._table))
        cr.execute(f"SELECT id FROM {self._table}")
        cr.execute("SELECT id FROM {}".format(self._table))

    def no_injection_constants(self, cr):
        # Not flagged: constants can't be injected.
        cr.execute("SELECT id FROM res_partner WHERE name = '%s'" % "admin")
        query = "SELECT id FROM " + "res_partner"
        cr.execute(query)

    def no_injection_psycopg2(self, cr, name):
        # Not flagged: psycopg2.sql composes identifiers safely.
        cr.execute(sql.SQL("SELECT id FROM {}").format(sql.Identifier(self._table)))
        query = sql.SQL("SELECT id FROM {}").format(sql.Identifier(self._table))
        cr.execute(query)

    def no_injection_other_cursor(self, other, name):
        # Not flagged: not a known cursor expression.
        other.execute("SELECT id FROM res_partner WHERE name = '%s'" % name)
