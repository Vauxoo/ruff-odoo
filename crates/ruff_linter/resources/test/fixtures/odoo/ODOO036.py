class MyModel(models.Model):
    _inherit = "my.model"

    def write(self, vals):
        return super().create(vals)

    def create(self, vals):
        return super().create(vals)

    def enqueue_cache_job(self):
        return super().refresh()
