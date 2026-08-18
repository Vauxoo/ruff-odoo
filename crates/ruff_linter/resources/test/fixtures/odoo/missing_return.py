class MyModel(models.Model):
    _inherit = "my.model"

    def write(self, vals):
        super().write(vals)

    def unlink(self):
        return super().unlink()

    def create(self, vals):
        if vals:
            return super().create(vals)
        return {}

    def action_do(self):
        super().action_do()

    # The call is the last statement, so the `return` can go in front of it even though
    # other statements run first.
    def copy(self, default=None):
        self.check_access("read")
        super(MyModel, self).copy(default)

    # Storing the result and doing nothing with it: only the author knows whether the
    # variable, `super()`'s value, or something else should be returned.
    def default_get(self, fields):
        res = super().default_get(fields)
        res["name"] = "x"

    # The call is the last statement of the `if`, not of the method, so inserting the
    # `return` there would change what the other branch does.
    def onchange_partner(self):
        if self.partner_id:
            super().onchange_partner()

    # Prepending the `return` here would skip the logging call.
    def action_confirm(self):
        super().action_confirm()
        _logger.info("confirmed")

    # `super()` is only an argument: the value of the trailing call is not the base
    # implementation's, so the rule leaves the choice to the author.
    def fields_get(self, allfields=None):
        dict(super().fields_get(allfields))

    # The trailing call is `action_do()`, not `super()`'s: its value is the one that would
    # be returned, so the choice stays with the author here too.
    def create_and_do(self, vals):
        super().create(vals).action_do()
