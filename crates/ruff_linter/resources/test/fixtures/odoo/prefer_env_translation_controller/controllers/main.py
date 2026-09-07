from odoo import _, http
from odoo.addons.website_sale.controllers.main import WebsiteSale


class MyController(http.Controller):
    @http.route("/page", auth="public")
    def page(self):
        # Reported, and fixed, only from Odoo 19.0 on, the version that gave `Controller` its
        # `env`. On 18.0 there is no `self.env` here to recommend, so nothing is reported.
        return _("in a controller")


# No `Controller` base and no route of its own: the file importing `odoo.http`, plus sitting
# in `controllers/`, is the only local evidence there is.
class InheritedController(WebsiteSale):
    def _prepare_values(self):
        return _("in an inherited controller")


# A data structure sharing the file with HTTP code is not a controller: no `self.env`.
class Store:
    def label(self):
        return _("in a baseless helper")


# Neither is an exception.
class MyError(Exception):
    def label(self):
        return _("in an exception")
