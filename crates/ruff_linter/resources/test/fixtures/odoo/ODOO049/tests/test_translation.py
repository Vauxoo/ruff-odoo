class TestSaleOrder:
    def test_confirm(self):
        # Not flagged: chatter text in tests is not user-facing.
        self.order.message_post(body="Order confirmed")
