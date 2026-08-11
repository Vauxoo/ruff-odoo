_("%s of %s")
_("%(count)s of %(total)s")
_("{} and {}")
_("only one %s")

# Fixable: values passed to the call as names or dotted attribute chains.
self.env._("Donation (%s) is (%s)", tier.name, tier.campaign_id.name)
_("%s of %s", count, total)
_lt("%d of %d", count, total)
self.env._("%s discount: %.2f", name, discount)
_("%s and %s", value, value)

# Not fixable: the values are interpolated outside the call.
_("%s of %s") % (count, total)
# Not fixable: an argument has no obvious name (call, literal, subscript).
_("%s of %s", get_count(), total)
_("%s of %s", count, values[0])
# Not fixable: argument count doesn't match the placeholder count.
_("%s of %s", count)
# Not fixable: distinct expressions colliding on the same derived name.
_("%s of %s", a.b, a_b)
# Not fixable: keyword arguments already present.
_("%s of %s", count, total=total)
