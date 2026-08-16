class MyModel(models.Model):
    _inherit = "my.model"

    def my_method(self):
        return _("old translated")

    def other_method(self):
        return _lt("also old")

    def already_fixed(self):
        return self.env._("ok")


def outside_method():
    return _("not fixable, no self in scope")
