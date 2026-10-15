from odoo import _, _lt, http, models
from odoo import _ as lt

# No fix: `self.env` needs a `self`, and there is none out here.
MODULE_LEVEL = _("at module level")

# No diagnostic: `_lt` is how a constant gets translated where there is no `env`.
LABELS = {"greeting": _lt("Hello")}


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

    def lazy_translation(self):
        # No diagnostic: `self.env._` would translate the term now instead of when it is
        # turned into text, so it is no replacement for `_lt`.
        return _lt("still lazy")

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


# A `Controller` base outside an addon is not a controller: the detector answers from the
# addon a file belongs to, and this fixture belongs to none. The controller cases live in
# `prefer_env_translation_controller/`, which has a manifest.
class MyController(http.Controller):
    @http.route("/page", auth="public")
    def page(self):
        return _("in a controller outside an addon")
