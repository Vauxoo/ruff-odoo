# Paid app (has "price") missing "currency", "images", and "support": flagged for each.
{
    "name": "Incomplete app",
    "license": "LGPL-3",
    "price": 100,
}

# Paid app with every store key present: ok.
{
    "name": "Complete app",
    "license": "LGPL-3",
    "price": 100,
    "currency": "EUR",
    "images": ["static/description/banner.png"],
    "support": "support@example.com",
}

# Paid app missing only "support": flagged once.
{
    "name": "Almost complete app",
    "license": "LGPL-3",
    "price": 100,
    "currency": "EUR",
    "images": ["static/description/banner.png"],
}

# No "price" key: not a paid app, the store keys are not required.
{
    "name": "Community module",
    "license": "LGPL-3",
}
