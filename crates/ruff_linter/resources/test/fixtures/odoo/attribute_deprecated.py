class MyModel(models.Model):
    _inherit = "my.model"

    _defaults = {"active": True}
    _columns = {}
    length = 10
    normal_attr = 5


class OrdinaryPythonClass:
    _defaults = {"active": True}
