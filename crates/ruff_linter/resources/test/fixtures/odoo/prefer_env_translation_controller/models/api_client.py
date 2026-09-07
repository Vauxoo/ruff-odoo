"""The location half of the controller test, pinned from the failing side.

Every class here passes the file-level signal — the file imports `odoo.http`, none is a
model, all have a base, none is a builtin or a test case — and none of them is a controller.
What keeps them out is that this file is neither under `controllers/` nor named
`controller*.py`.

Modelled on `OCA/rest-framework@a34479e4`'s `base_rest/restapi.py`, where
`CerberusValidator(RestMethodParam)` calls `_()` in a `self` method, has no `env`, and would
otherwise be handed a safe fix rewriting it to `self.env._(...)`.
"""

import abc

from odoo import _, http
from odoo.exceptions import UserError


class RestMethodParam(abc.ABC):
    pass


class CerberusValidator(RestMethodParam):
    def from_params(self, service, params):
        # Reported without a fix: no `self.env` here to rewrite to.
        raise UserError(_("BadRequest %s") % params)


class ApiClient(http.Dispatcher):
    def label(self):
        return _("not a controller either")
