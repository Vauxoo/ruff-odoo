class MyModel(models.Model):
    _inherit = "my.model"

    name = fields.Char(string=_("Name"))
    label = fields.Char("Label")
    help_text = fields.Char(help=_lt("Some help"))
    # Flagged, but no fix: more than one argument.
    title = fields.Char(string=_("Title %s", suffix))
