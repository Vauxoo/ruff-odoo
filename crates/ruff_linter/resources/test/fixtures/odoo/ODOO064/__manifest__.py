{
    "name": "Wrong version formats",
    # Flagged: the series separator is an underscore, not a dot.
    "version": "8_0.1.0.0",
}

# Flagged: only two components.
{
    "version": "1.0",
}

# Flagged: trailing garbage after the last component.
{
    "version": "8.0.1.0.0foo",
}

# Flagged: four components, the module part is short one.
{
    "version": "17.0.1.0",
}

# Flagged: six components.
{
    "version": "17.0.1.0.0.1",
}

# Flagged: an empty component.
{
    "version": "17..1.0.0",
}

# Not flagged: five numeric components.
{
    "version": "17.0.1.0.0",
}

# Not flagged: the series is only checked when `odoo-version` is configured.
{
    "version": "8.0.1.0.0",
}

# Not flagged: an absent or empty version is `manifest-required-key`'s business.
{
    "version": "",
}

# Not flagged: a non-literal version can't be checked.
{
    "version": VERSION,
}
