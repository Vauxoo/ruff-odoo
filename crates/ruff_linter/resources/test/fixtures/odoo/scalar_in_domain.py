from odoo import fields, models


class OrphanOrphan(models.Model):
    _name = "orphan.orphan"

    country_id = fields.Many2one("res.country")
    campaign_ids = fields.Many2many("orphan.campaign", domain=[("state", "in", "open")])

    def positive_id(self):
        # ODW9505: `.id` is the id of one record.
        for orphan in self:
            self.env["orphan.group"].search([("country_ids", "in", orphan.country_id.id)], limit=1)
            self.env["orphan.group"].search_count([("country_ids", "not in", orphan.country_id.id)])
        self.env["orphan.group"].search_read([("country_ids", "in", self.env.company.id)])
        self.filtered_domain([("country_ids", "in", self.country_id.id)])
        return self.env["orphan.group"]._search([("country_ids", "in", self.id)])

    def positive_uid(self):
        # ODW9505: `uid` is the id of the user.
        self.env["res.partner"].search([("starred_user_ids", "in", self.env.uid)])
        self.env["res.partner"].search([("starred_user_ids", "not in", self._uid)])
        return [("starred_user_ids", "in", uid)]

    def positive_literals(self):
        # ODW9505: a literal is a scalar; a falsy one turns the condition into a constant.
        self.env["res.partner"].search([("state", "in", "draft")])
        self.env["res.partner"].search([("sequence", "not in", 5)])
        self.env["res.partner"].search([("active", "in", True)])
        self.env["res.partner"].search([("parent_id", "in", False)])
        self.env["res.partner"].search([("parent_id", "in", None)])
        self.env["res.partner"].search([("parent_id", "not in", 0)])
        self.env["res.partner"].search([("name", "in", "")])
        self.env["res.partner"].search([("sequence", "in", -1)])

    def positive_domain_variable(self, partner):
        # ODW9505: any list of conditions is looked at, not only the ones passed to `search`.
        domain = [("company_ids", "in", partner.company_id.id), ("active", "=", True)]
        domain += [("category_id", "in", partner.category_id.id)]
        return self.env["res.partner"].search(domain)

    def positive_name_assigned_once(self):
        # ODW9505: the only assignment of `cause_id` is a record id.
        cause_id = self.campaign_id.cause_id.id
        return self.env["product.template"].search([("cause_ids", "in", cause_id)], limit=1)

    def positive_nested(self):
        # ODW9505: inside `|`, `&` and `!` groups, and inside a condition of a tuple.
        return [
            "|",
            ("cause_ids", "in", self.cause_id.id),
            "&",
            ("state", "=", "open"),
            ("cause_ids", "in", self.other_cause_id.id),
        ]

    def positive_list_condition(self):
        # ODW9505: a condition written as a list is the same condition.
        return [["cause_ids", "in", self.cause_id.id]]


def positive_module_level(partner):
    # ODW9505
    return [("partner_ids", "in", partner.id)]


class Negative(models.Model):
    _name = "negative"

    def negative_collections(self, partner, partners, ids):
        # A list, tuple or set of any size, empty or not.
        self.search([("country_ids", "in", [partner.country_id.id])])
        self.search([("country_ids", "in", (1, 2))])
        self.search([("country_ids", "in", {1, 2})])
        self.search([("country_ids", "not in", [])])
        self.search([("country_ids", "in", [partner.id, self.id])])
        self.search([("state", "in", ["draft", "open"])])
        self.search([("state", "not in", ("draft", "open"))])

    def negative_recordsets_and_ids(self, partner, partners, ids):
        # `.ids`, a recordset, a call, a comprehension or a subscript: not known to be a scalar.
        self.search([("country_ids", "in", partner.country_id.ids)])
        self.search([("country_ids", "in", partner.country_id)])
        self.search([("country_ids", "in", partners.mapped("country_id").ids)])
        self.search([("country_ids", "in", partners.country_id)])
        self.search([("country_ids", "in", self.env.companies.ids)])
        self.search([("id", "in", ids)])
        self.search([("id", "in", list(ids))])
        self.search([("id", "in", [rec.id for rec in partners])])
        self.search([("id", "in", ids[0])])
        self.search([("id", "in", ids.get("a"))])
        self.search([("id", "in", partner.id or [])])
        self.search([("id", "in", [partner.id] if partner else [])])
        self.search([("id", "in", self.env["res.partner"]._search([]))])
        self.search([("id", "in", SQL("SELECT 1"))])

    def negative_names(self, company_id, partner_id, active_id):
        # A Python name is not guessed from its spelling: `company_id` may be a recordset.
        company_id = self.env.company
        self.search([("company_id", "in", company_id)])
        self.search([("partner_id", "in", partner_id)])
        self.search([("partner_id", "in", active_id)])

    def negative_name_assigned_twice(self, partner, flag):
        # The name has two assignments: not known which one reaches the condition.
        value = partner.id
        if flag:
            value = partner.ids
        return self.search([("partner_ids", "in", value)])

    def negative_name_assigned_to_a_list(self, partner):
        ids = [partner.id]
        return self.search([("partner_ids", "in", ids)])

    def negative_operators(self, partner):
        # Only `in` and `not in` expect a collection.
        self.search([("partner_id", "=", partner.id)])
        self.search([("partner_id", "!=", partner.id)])
        self.search([("partner_id", "child_of", partner.id)])
        self.search([("partner_id", "parent_of", partner.id)])
        self.search([("name", "ilike", "a")])
        self.search([("partner_id", "any", [("id", "=", partner.id)])])
        self.search([("sequence", ">", 5)])

    def negative_not_a_condition(self, partner):
        # Four items, two items, or a tuple of operators.
        operators = ("in", "not in", "any")
        pair = ("country_ids", "in")
        four = ("country_ids", "in", partner.id, 4)
        membership = partner.id in [1, 2]
        field = "country_ids"
        dynamic = (field, "in", partner.id)
        dynamic_operator = ("country_ids", operators[0], partner.id)
        return operators, pair, four, membership, dynamic, dynamic_operator

    def negative_nested_condition(self, partner):
        # The subdomain of `any` is a list of conditions; the scalar inside is still reported.
        return [("partner_id", "any", [("id", "=", partner.id)])]


class StringDomain(models.Model):
    _name = "string.domain"

    company_id = fields.Many2one("res.company")
    # ODW9505: a field evaluates its string domain with the fields of the record.
    journal_id = fields.Many2one("account.journal", domain="[('company_ids', 'in', company_id)]")
    user_ids = fields.Many2many("res.users", domain="[('partner_ids', 'not in', uid)]")
    cause_id = fields.Many2one(
        "res.cause",
        domain="""
            ['|', ('country_ids', 'in', active_id), ('state', 'in', 'open')]
        """,
    )


class NegativeStringDomain(models.Model):
    _name = "negative.string.domain"

    company_id = fields.Many2one("res.company")
    company_ids = fields.Many2many("res.company")
    # A list, a field that is a list, another operator, or a string that is not a domain.
    ok_list = fields.Many2one("res.partner", domain="[('company_ids', 'in', [company_id])]")
    ok_field = fields.Many2one("res.partner", domain="[('company_ids', 'in', company_ids)]")
    ok_operator = fields.Many2one("res.partner", domain="[('company_id', '=', company_id)]")
    ok_other_argument = fields.Many2one("res.partner", help="[('company_ids', 'in', company_id)]")
    ok_dynamic = fields.Many2one("res.partner", domain="[('company_ids', 'in', company_id.ids)]")
    ok_f_string = fields.Many2one("res.partner", domain=f"[('company_ids', 'in', {1})]")
    ok_concatenated = fields.Many2one(
        "res.partner",
        domain="[('company_ids', 'in', " "company_id)]",
    )
    ok_escaped = fields.Many2one("res.partner", domain="[('company_ids', 'in', \x63ompany_id)]")
    ok_not_python = fields.Many2one("res.partner", domain="%(placeholder)s")
