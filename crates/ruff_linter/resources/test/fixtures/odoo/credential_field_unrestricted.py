from odoo import fields, models


class ResCompany(models.Model):
    _inherit = "res.company"

    # Flagged: no `groups`.
    twilio_auth_token = fields.Char(password=True)
    clover_api_key = fields.Char(string="Clover API key")
    mkc_client_secret = fields.Char()
    # Flagged: laxer than the system group.
    sendgrid_api_key = fields.Char(groups="base.group_user")
    # Flagged: any group of the list grants access.
    address_api_key = fields.Char(groups="base.group_user,base.group_system")
    # Flagged: `password=True` only masks the widget, whatever the name says.
    pin = fields.Char(password=True)
    # Flagged: other field types that hold a credential.
    private_key = fields.Text()
    key_file = fields.Binary(password=True)
    # Flagged: the name is the whole suffix.
    secret = fields.Char()

    # OK: restricted to the system group.
    stripe_secret_key = fields.Char(password=True, groups="base.group_system")
    wh_secret = fields.Char(groups="base.group_system, base.group_system")
    # OK: not a credential.
    secret_name = fields.Char()
    api_key_count = fields.Integer()
    publishable_key = fields.Char()
    my_secretary = fields.Char()
    # OK: not a text field.
    api_key_id = fields.Many2one("res.users.apikeys")
    # OK: `groups` is not a literal, so its value can not be known.
    other_api_key = fields.Char(groups=GROUPS)
    # OK: the options can carry the group.
    spread_api_key = fields.Char(**OPTIONS)
    # OK: `password=False` and no credential name.
    nickname = fields.Char(password=False)


class ResConfigSettings(models.TransientModel):
    _inherit = "res.config.settings"

    # OK: transient, nothing is stored by the model itself.
    clover_api_key = fields.Char(related="company_id.clover_api_key")


class NotAModel:
    # OK: not an Odoo model.
    api_key = fields.Char()
