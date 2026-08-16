class MyModel(models.Model):
    _inherit = "my.model"

    name = fields.Char(string=_("Name"))
    label = fields.Char("Label")
