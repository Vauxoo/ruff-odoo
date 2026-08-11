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

# Fixable: the name is derived from inside the expression — the first nameable call
# argument, the callee, or the subscript base plus its index.
self.env._(
    "The following fields are required:\n - %s\nFor the record %s",
    "\n - ".join(fields_string),
    sale.display_name,
)
_("%s of %s", get_count(), total)
_("%s and %s", sale.mapped("name"), count)
_("%s of %s", values[0], values[1])
_("%s of %s", vals["name"], vals[key])

# Not fixable: the values are interpolated outside the call.
_("%s of %s") % (count, total)
# Not fixable: no identifier anywhere in the argument.
_("%s of %s", count + 1, total)
_("%s of %s", 42, total)
# Not fixable: argument count doesn't match the placeholder count.
_("%s of %s", count)
# Not fixable: distinct expressions colliding on the same derived name.
_("%s of %s", a.b, a_b)
# Not fixable: keyword arguments already present.
_("%s of %s", count, total=total)
