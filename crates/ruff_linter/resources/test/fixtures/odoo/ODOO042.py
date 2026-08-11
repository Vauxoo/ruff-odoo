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
self.env._(
    "%s -> %s (From %s to %s)",
    record.delegated_from_id.display_name,
    record.delegated_id.display_name,
    format_date(self.env, record.start_date),
    format_date(self.env, record.end_date),
)
_("%s and %s", sale.mapped("name"), count)
_("%s of %s", values[0], values[1])
_("%s of %s", vals["name"], vals[key])
_(
    "Partners:\n - %s\n\n Domain used %s",
    "\n - ".join(["%s - %s" % (p.ref, p.name) for p in partners_werror]),
    domain,
)
_("date '%s'.%s", date_to_convert, self.env._(" In: %s.", title) if title else "")
_("closure %s already exists for %s", refundable_closures[0].name, operation.name)
_("%s must have a coverage greater than 0%% and up to %s%%", line.name, line.program_id.coverage * 100)
_("%s of %s", count + 1, total)
_(
    "Failed to parse dates for absence %s (Start: %s, End: %s)",
    hcm_id_leave,
    oracle_leave.get("startDateTime"),
    oracle_leave.get("endDateTime"),
)
_("Replacement of %s: %s", record.name, datetime.now().strftime("%d/%m/%Y %H:%M:%S"))
_("adjustment of %s by %s", line.name, f"{entry_amount:,.2f}")

# A literal '%' before punctuation is not a printf conversion: no diagnostic at all.
_("coverage between 75.01% & 90%. It is only allowed with an emergency declaration")

# Not fixable: the values are interpolated outside the call.
_("%s of %s") % (count, total)
# Not fixable: no identifier anywhere in the argument.
_("%s of %s", 42, total)
# Not fixable: argument count doesn't match the placeholder count.
_("%s of %s", count)
# Not fixable: distinct expressions colliding on the same derived name.
_("%s of %s", a.b, a_b)
# Not fixable: keyword arguments already present.
_("%s of %s", count, total=total)
