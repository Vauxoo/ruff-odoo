class MyModel(models.Model):
    _inherit = "my.model"

    def write(self, vals):
        return True

    def write(self, vals):
        return super().write(vals)

    def create(self, vals):
        if vals:
            return super().create(vals)
        return {}

    def action_confirm(self):
        return True

    @api.model
    def default_get(self, fields_list):
        res = super().default_get(fields_list)
        res.update({})
        return res


class SaleOrderLine(models.Model):
    _inherit = "sale.order.line"

    # `analytic.mixin.init` creates a gin index for `sale_order_line`; dropping the
    # `super()` call drops the index.
    def init(self):
        self.env.cr.execute("CREATE INDEX ...")


class SaleOrderLineChained(models.Model):
    _inherit = "sale.order.line"

    def init(self):
        super().init()
        self.env.cr.execute("CREATE INDEX ...")


class MyDelegatedModel(models.Model):
    _name = "my.delegated.model"
    _inherits = {"my.model": "my_model_id"}

    def init(self):
        self.env.cr.execute("CREATE INDEX ...")


class MyNewModel(models.Model):
    _name = "my.new.model"

    # A brand-new model has no inherited `init` to chain to: `models.Model.init` is a no-op.
    def init(self):
        self.env.cr.execute("CREATE INDEX ...")

    def write(self, vals):
        return True


def write(vals):
    return True
