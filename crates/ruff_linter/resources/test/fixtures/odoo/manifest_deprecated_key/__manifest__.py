{
    "description": "Does things.",
    "name": "First",
}

{
    "name": "Second",
    "description": "Does things.",
}

{
    "description": "Does things.",
}

{
    "name": "Fourth",
}

{
    "name": "Active",
    "active": True,
}

{
    "name": "Qweb",
    "qweb": ["static/src/xml/base.xml"],
}

{
    "name": "Replacements",
    "auto_install": True,
    "assets": {"web.assets_backend": ["my_module/static/src/xml/base.xml"]},
}

{
    "name": "Multiline Description",
    "description": """
        A long legacy description spanning
        several lines: the diagnostic must point at the key,
        not highlight this whole block.
    """,
}
