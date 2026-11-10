from datetime import timedelta

from dateutil.relativedelta import relativedelta
from odoo import fields, models


class DonationReport(models.Model):
    _name = "donation.report"

    # ODW9506: a field domain written as a string is evaluated in the browser, in UTC.
    partner_id = fields.Many2one(
        "res.partner",
        domain="[('create_date', '>=', time.strftime('%Y-%m-%d 00:00:00')), ('create_date', '<=', time.strftime('%Y-%m-%d 23:59:59'))]",
    )
    today_id = fields.Many2one(
        "res.partner",
        domain="[('settlement_date', '=', context_today().strftime('%Y-%m-%d'))]",
    )
    pending_id = fields.Many2one(
        "res.partner",
        domain="[('date_begin', '<=', context_today()), ('date_end', '>=', context_today())]",
    )
    doubled_id = fields.Many2one(
        "res.partner",
        domain="[('create_date', '>=', time.strftime('%%Y-%%m-%%d 00:00:00'))]",
    )
    recent_id = fields.Many2one(
        "res.partner",
        domain="[('date', '>', (context_today() - relativedelta(days=3)).strftime('%Y-%m-%d'))]",
    )
    month_id = fields.Many2one(
        "res.partner",
        domain="[('date', '>=', (context_today() - relativedelta(months=1)).strftime('%Y-%m-%d')), ('date', '<=', (context_today() + datetime.timedelta(weeks=1)))]",
    )
    now_id = fields.Many2one(
        "res.partner",
        domain="[('create_date', '<=', datetime.datetime.now()), ('write_date', '>', datetime.datetime.now() - datetime.timedelta(days=1))]",
    )
    unreadable_id = fields.Many2one(
        "res.partner",
        domain="[('date', '>=', context_today().strftime('%Y-%m-01')), ('date', '<', (context_today() + relativedelta(months=1, day=1)))]",
    )
    different_id = fields.Many2one(
        "res.partner",
        domain="[('date', '!=', context_today()), ('date', '>=', datetime.date.today() - relativedelta(years=1))]",
    )

    # Not reported: a domain in Python code is evaluated by the server, where the day of the
    # user is `context_today(self)`.
    def search_today(self):
        self.search([("date", ">=", fields.Date.context_today(self))])
        self.search([("date", "<=", fields.Date.today() + relativedelta(days=3))])
        self.search([("create_date", ">=", fields.Datetime.now() - timedelta(days=1))])
        return [("date", "=", "today")]

    # Not reported: the relative syntax, other values, and strings that are not read.
    clean_id = fields.Many2one(
        "res.partner",
        domain="[('create_date', '>=', 'today'), ('create_date', '<', 'today +1d'), ('date', '=', 'today -1d')]",
    )
    other_id = fields.Many2one(
        "res.partner",
        domain="[('user_id', '=', uid), ('date', '=', date_field), ('name', '=', time.strftime), ('state', 'in', ['a'])]",
    )
    label_id = fields.Many2one(
        "res.partner",
        domain="[('name', 'ilike', context_today().strftime('%Y-%m-%d'))]",
    )
    fstring_id = fields.Many2one(
        "res.partner",
        domain=f"[('date', '>=', context_today().strftime('%Y-%m-%d')){'x'}]",
    )
    concatenated_id = fields.Many2one(
        "res.partner",
        domain="[('date', '>=', context_today()),"
        "('date', '<=', context_today())]",
    )
