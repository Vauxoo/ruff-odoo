from odoo import models

MODEL_NAME = "res.partner"


# OK: the class name is the model name in CamelCase.
class EventEvent(models.Model):
    _name = "event.event"


class SaleOrder(models.Model):
    _inherit = "sale.order"


class MailThread(models.AbstractModel):
    _name = "mail.thread"


# OK: an underscore in the model keeps its place and still capitalises what follows.
class IrActionsAct_Window(models.Model):
    _name = "ir.actions.act_window"


class L10n_CzTax_Office(models.Model):
    _name = "l10n_cz.tax_office"


# OK: differs only in capitalisation, the way Odoo writes its own acronyms.
class AccountEdiUBL(models.AbstractModel):
    _name = "account.edi.ubl"


class AccountEdiXmlUBL20(models.AbstractModel):
    _name = "account.edi.xml.ubl_20"


# OK: differs only in where the underscores fall.
class ImLivechatChannelMemberHistory(models.Model):
    _name = "im_livechat.channel.member.history"


class L10nHrEdiAddendum(models.Model):
    _name = "l10n_hr_edi.addendum"


# OK: `_name` wins over `_inherit` when both are present.
class AccountMove(models.Model):
    _name = "account.move"
    _inherit = ["mail.thread", "mail.activity.mixin"]


# OK: a list-valued `_inherit` names no single model, so the class name is unconstrained.
class WhateverName(models.Model):
    _inherit = ["mail.thread", "res.partner"]


# OK: `base` is the registry root, not a model to be named after.
class Base(models.AbstractModel):
    _inherit = "base"


# OK: `_name` is not a string literal, so the model cannot be resolved -- and `_inherit` must
# not be used as a fallback here, it would name the wrong model.
class Anything(models.Model):
    _name = MODEL_NAME
    _inherit = "res.users"


# OK: not an Odoo model.
class Partner:
    _name = "res.partner"


class PlainClass(object):
    _inherit = "stock.picking"


# ODW9503: named after another model entirely.
class Partner(models.Model):
    _inherit = "res.partner"


class Picking(models.Model):
    _inherit = "stock.picking"


class Uom(models.Model):
    _inherit = "uom.uom"


# ODW9503: the words are right but in the wrong order.
class MessageMailLinkPreview(models.Model):
    _name = "mail.message.link.preview"


# ODW9503: a leftover from before the model was renamed.
class EventMailRegistration(models.Model):
    _name = "event.mail.slot"


class HrDepartureWizard(models.TransientModel):
    _name = "hr.version.wizard"


# ODW9503: missing the module prefix the model carries.
class PeppolClarification(models.Model):
    _name = "account.peppol.clarification"
