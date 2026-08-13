from odoo.tools import clean_context

ctx = {"key": "value"}
self.with_context(ctx)
self.with_context(key="value")
self.with_context(**ctx)
self.with_context(clean_context(self.env.context))
