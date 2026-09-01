import odoo
from odoo import api, models
from odoo.api import returns


# --- positives: the decorator is lost through `_inherit` ---


class ProjectTask(models.Model):
    _inherit = "project.task"

    def message_post(self, **kwargs):  # ODW9503
        return super().message_post(**kwargs)


class MailThread(models.AbstractModel):
    _inherit = "mail.thread"

    def message_notify(self, **kwargs):  # ODW9503 (17.0+)
        return super().message_notify(**kwargs)


class ResPartner(models.Model):
    _inherit = "res.partner"

    def find_or_create(self, emails, **kwargs):  # ODW9503
        return super().find_or_create(emails, **kwargs)

    def main_partner(self):  # ODW9503 (16.0-17.0 only)
        return super().main_partner()


class DiscussChannel(models.Model):
    _inherit = ["discuss.channel"]

    def channel_get(self, partners_to=None, **kwargs):  # ODW9503, 17.0 != 18.0 decorator
        return super().channel_get(partners_to=partners_to, **kwargs)


class IrFilters(models.Model):
    _name = "ir.filters"
    _inherit = "ir.filters"

    def create_or_replace(self, vals):  # ODW9503
        return super().create_or_replace(vals)


# --- negatives: already decorated, in every spelling ---


class DecoratedAttribute(models.Model):
    _inherit = "project.task"

    @api.returns("mail.message", lambda value: value.id)
    def message_post(self, **kwargs):
        return super().message_post(**kwargs)


class DecoratedImported(models.Model):
    _inherit = "project.task"

    @returns("mail.message", lambda value: value.id)
    def message_post(self, **kwargs):
        return super().message_post(**kwargs)


class DecoratedDotted(models.Model):
    _inherit = "project.task"

    @odoo.api.returns("mail.message", lambda value: value.id)
    def message_post(self, **kwargs):
        return super().message_post(**kwargs)


# --- negatives: BaseModel provides the decorator, `Meta` propagates it ---


class BaseModelMethods(models.Model):
    _inherit = "sale.order"

    def copy(self, default=None):
        return super().copy(default)

    def create(self, vals_list):
        return super().create(vals_list)

    def search(self, domain, **kwargs):
        return super().search(domain, **kwargs)

    def exists(self):
        return super().exists()


# --- negatives: near misses ---


class NotDecoratedInCore(models.AbstractModel):
    _inherit = "mail.thread"

    def message_post_with_source(self, source_ref, **kwargs):
        return super().message_post_with_source(source_ref, **kwargs)


class BrandNewModel(models.Model):
    _name = "my.own.model"

    def message_post(self, **kwargs):
        return {"posted": True}


class WrongModelForTheMethod(models.Model):
    _inherit = "sale.order"

    def find_or_create(self, values):
        return self.create(values)


class PrivateMethod(models.Model):
    _inherit = "stock.warehouse"

    def _get_all_routes(self):
        return super()._get_all_routes()


class NotAnOdooModel:
    _inherit = "mail.thread"

    def message_post(self, **kwargs):
        return None
