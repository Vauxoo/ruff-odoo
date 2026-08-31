# Already sorted: nothing to report.
{
    "name": "Sorted",
    "depends": [
        "account",
        "base",
        "sale",
    ],
}

{"name": "Sorted Inline", "depends": ["account", "base", "sale"]}

# Nothing to sort.
{"name": "Empty", "depends": []}

{"name": "Single", "depends": ["sale"]}

# No "depends" key at all.
{"name": "No Depends", "data": ["views/sale_views.xml"]}

# Plain unsorted list, one entry per line.
{
    "name": "Unsorted",
    "depends": [
        "sale",
        "account",
        "base",
    ],
}

# Every comment travels with the entry it belongs to: the block written above it, and the
# one trailing it on its own line.
{
    "name": "Unsorted With Comments",
    "depends": [
        # sale has to come first because of the widget
        # (second line of the same comment block)
        "sale",
        "account",  # analytic distribution
        "base",
    ],
}

# The last entry has no trailing comma; the rewrite always leaves one behind.
{
    "name": "Unsorted No Trailing Comma",
    "depends": [
        "sale",
        "account",
        "base"
    ],
}

# The entry that lacks the comma is also the one that stays last: it still gets one.
{
    "name": "Unsorted Last Entry Stays Last",
    "depends": [
        "base",
        "account",
        "sale"
    ],
}

# A comment on the opening bracket's line annotates the list, not an entry, so it stays
# there; a comment block written after the last entry stays at the end.
{
    "name": "Unsorted Bracket Comments",
    "depends": [  # keep me sorted
        "sale",
        "account",
        # TODO: drop base once the glue module lands
    ],
}

# Several entries on one line are spread out to one per line.
{
    "name": "Unsorted Two Per Line",
    "depends": [
        "sale", "account",
        "base",
    ],
}

# The opening bracket's line already carries an entry: rewritten with the bracket alone.
{
    "name": "Unsorted Hanging Bracket",
    "depends": ["sale",
                "account"],
}

# The closing bracket shares the last entry's line: rewritten onto its own line.
{
    "name": "Unsorted Closing Bracket",
    "depends": [
        "sale",
        "account"],
}

# Single-line list: reordered in place, layout and missing trailing comma left alone.
{"name": "Unsorted Inline", "depends": ["sale", "account", "base"]}

# Duplicates keep their relative order.
{
    "name": "Unsorted Duplicated",
    "depends": [
        "sale",
        "account",
        "sale",
    ],
}

# `_` sorts before a letter, exactly like Python's `sorted()`.
{
    "name": "Unsorted Underscore",
    "depends": [
        "salemodule",
        "sale_stock",
    ],
}

# A blank line groups the entries; reordering across it would scramble the grouping, so
# this is reported without a fix.
{
    "name": "Unsorted Blank Line",
    "depends": [
        "sale",

        "account",
    ],
}

# Two entries share the line the comment trails: there is no telling which one it was
# written about, so this is reported without a fix.
{
    "name": "Unsorted Two Per Line Commented",
    "depends": [
        "sale", "account",  # which one is this about?
        "base",
    ],
}

# An entry spanning two lines: reported without a fix.
{
    "name": "Unsorted Split Entry",
    "depends": [
        "sale",
        "acc"
        "ount",
    ],
}

# An entry that is not a plain string literal: nothing to order, nothing to report.
{
    "name": "Not A String",
    "depends": [
        "sale",
        ("account",),
    ],
}

# Only the manifest's own top-level dict is checked, never a nested one.
{
    "name": "Nested",
    "depends": ["account", "base"],
    "assets": {
        "web.assets_backend": [
            "sale/static/src/js/b.js",
            "sale/static/src/js/a.js",
        ],
    },
}
