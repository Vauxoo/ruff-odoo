from odoo import _, _lt, http, models
from odoo import _ as lt
from odoo.addons.website_sale.controllers.main import WebsiteSale

# No fix: `self.env` needs a `self`, and there is none out here.
MODULE_LEVEL = _("at module level")


def outside_a_class():
    return _("in a plain function")


class NotOdoo:
    def method(self):
        # No fix: the class is not an Odoo one, so its `self` carries no `env`.
        return _("in a plain class")


class MyModel(models.Model):
    _inherit = "my.model"

    # No fix: a call in the class body is an attribute, not a method.
    LABEL = _("class attribute")

    def my_method(self):
        return _("old translated")

    def other_method(self):
        return _lt("also old")

    def imported_under_another_name(self):
        # What the function resolves to is what matters, not what it is called here.
        return lt("aliased import")

    def nested_calls(self):
        return _("outer %s", _("inner"))

    def already_fixed(self):
        return self.env._("ok")

    @staticmethod
    def static_method():
        return _("no self")

    def nested_function(self):
        def inner():
            return _("no self in the inner function")

        return inner


class MyController(http.Controller):
    @http.route("/page", auth="public")
    def page(self):
        # Reported, and fixed, only from Odoo 19.0 on, the version that gave `Controller` its
        # `env`. On 18.0 there is no `self.env` here to recommend, so nothing is reported.
        return _("in a controller")


# A controller reached through another addon, with no `Controller` base and no route of its
# own. The file importing `odoo.http` is the only local evidence there is.
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
