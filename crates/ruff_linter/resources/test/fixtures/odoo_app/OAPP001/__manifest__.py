# Paid app (has "price") with a category outside the Odoo Apps store list: flagged.
{
    "name": "Bad category app",
    "price": 100,
    "category": "Tools",
}

# Paid app with an allowed category: ok.
{
    "name": "Good category app",
    "price": 100,
    "category": "Sales",
}

# No "price" key: not a paid app, the store category list does not apply.
{
    "name": "Community module",
    "category": "Tools",
}

# Paid app without a category: ok (nothing to validate).
{
    "name": "No category app",
    "price": 100,
}

# Paid app with an empty category: ok (pylint-odoo skips empty categories).
{
    "name": "Empty category app",
    "price": 100,
    "category": "",
}
