{
    "name": "My Module",
    "summary": "Does\nthings.",
}

{
    "name": "My Other Module",
    "summary": "Does things.",
}

{
    "name": "Whitespace Around Newline",
    "summary": "hola \n  moy",
}

{
    "name": "Triple Quoted",
    "summary": """
        Multi line
        summary
    """,
}

{
    "name": "Implicit Concatenation",
    "summary": "First line\n" "second line",
}

# Only the source layout is multi-line here: the concatenated value has no newline in it,
# so there is nothing to collapse and the summary must be left untouched.
{
    "name": "Implicit Concatenation Across Lines",
    "summary": "Add conditional mako template to any report "
    "on models that inherits comment.template.",
}

# A backslash continuation is also purely a source-layout newline: the value stays one line.
{
    "name": "Backslash Continuation",
    "summary": "Add conditional mako template to any report \
on models that inherits comment.template.",
}

{
    "name": "CRM Activity Automation",
    "summary": """
    Stage-triggered activity automation for CRM. When an opportunity
    moves to a configured stage, activities (calls, emails, meetings)
    are automatically created and assigned.
    """,
}

{"name": "Inline Dict", "summary": "Inline dict where the collapsed summary is way too long to fit\nwithin the configured maximum line length so it stays single-line anyway",}
