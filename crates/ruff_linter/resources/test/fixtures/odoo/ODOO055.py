# Test settings: prohibited-override-methods = ["action_post", "unlink"]


class AccountMove:
    def action_post(self):  # ODOO055
        return super().action_post()

    def unlink(self):  # ODOO055
        res = super().unlink()
        return res

    def write(self, vals):  # ok: not in the prohibited list
        return super().write(vals)

    def action_post_hook(self):  # ok: name not in the prohibited list
        return super().action_post_hook()

    def action_draft(self):  # ok: delegates to a different method
        return super().action_post_or_draft()


class SaleOrder:
    def action_post(self):  # ok: no super() delegation (pylint-odoo parity)
        return True


def action_post():  # ok: not a method
    return super().action_post()
