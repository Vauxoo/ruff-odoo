#!/usr/bin/env python3
"""Regenerate the per-version stubs of Odoo's ORM model methods.

The `OD` plugin binds call arguments against the parameter lists Odoo actually
declares, which change from one release to the next: `read_group` alone loses
`lazy`, gains `aggregates` and changes the meaning of its second positional
between 18.0 and 20.0. Rather than transcribe those signatures by hand, this
script reads them out of git revisions of odoo/odoo and writes one
signature-only Python file per version under
`crates/ruff_linter/resources/odoo/`, which the rule embeds with `include_str!`
and parses once.

The stubs are generated artifacts: regenerate them, do not hand-edit them.

    python3 scripts/generate_odoo_model_stubs.py --odoo ~/odoo/odoo

Odoo's ORM lived in `odoo/models.py` up to 18.0 and moved to the `odoo/orm/`
package in 19.0. There is no 20.0 branch yet, so its stub is cut from `master`
and the header records the exact commit it came from.
"""

from __future__ import annotations

import argparse
import ast
import pathlib
import subprocess
import sys

# Every version the rule ships a stub for, with the ref to read it from and the
# modules the ORM model classes live in at that ref.
VERSIONS = [
    ("16.0", "odoo/16.0", ["odoo/models.py"]),
    ("17.0", "odoo/17.0", ["odoo/models.py"]),
    ("18.0", "odoo/18.0", ["odoo/models.py"]),
    ("19.0", "odoo/19.0", ["odoo/orm/models.py", "odoo/orm/models_transient.py"]),
    ("20.0", "odoo/master", ["odoo/orm/models.py", "odoo/orm/models_transient.py"]),
]

# The classes whose methods any recordset exposes. `Model`, `AbstractModel` and
# `TransientModel` add almost nothing over `BaseModel`, but reading them keeps
# the stub correct wherever a method moved down the hierarchy between releases.
MODEL_CLASSES = ("BaseModel", "Model", "AbstractModel", "TransientModel")


def git_show(repo: pathlib.Path, ref: str, path: str) -> str | None:
    """Return the contents of `path` at `ref`, or `None` if it does not exist there."""
    result = subprocess.run(
        ["git", "show", f"{ref}:{path}"],
        capture_output=True,
        text=True,
        cwd=repo,
        check=False,
    )
    return result.stdout if result.returncode == 0 else None


def render_parameters(args: ast.arguments) -> str:
    """Render a parameter list, keeping only what deciding a call's validity needs.

    Annotations and default *values* are dropped — the check is about arity and
    parameter names, and a default expression would drag Odoo's module-level
    constants into a file that is never imported. `=...` still marks a parameter
    as optional, `/` and `*` still mark the positional-only and keyword-only
    boundaries, and `*args` / `**kwargs` are kept verbatim because they are what
    tells the rule to stop counting.
    """
    parts: list[str] = []
    positional = args.posonlyargs + args.args
    defaults = [None] * (len(positional) - len(args.defaults)) + list(args.defaults)
    for index, (arg, default) in enumerate(zip(positional, defaults, strict=True)):
        parts.append(f"{arg.arg}=..." if default is not None else arg.arg)
        if args.posonlyargs and index == len(args.posonlyargs) - 1:
            parts.append("/")
    if args.vararg:
        parts.append(f"*{args.vararg.arg}")
    elif args.kwonlyargs:
        parts.append("*")
    for arg, default in zip(args.kwonlyargs, args.kw_defaults, strict=True):
        parts.append(f"{arg.arg}=..." if default is not None else arg.arg)
    if args.kwarg:
        parts.append(f"**{args.kwarg.arg}")
    return ", ".join(parts)


def is_property(decorator: ast.expr) -> bool:
    """Whether `decorator` names `property` or `functools.cached_property`."""
    if isinstance(decorator, ast.Name):
        name = decorator.id
    elif isinstance(decorator, ast.Attribute):
        name = decorator.attr
    else:
        return False
    return name in ("property", "cached_property")


def collect(repo: pathlib.Path, ref: str, paths: list[str]) -> dict[str, ast.arguments]:
    """Collect the parameter list of every ORM model method defined at `ref`."""
    methods: dict[str, ast.arguments] = {}
    for path in paths:
        source = git_show(repo, ref, path)
        if source is None:
            continue
        for node in ast.parse(source).body:
            if not isinstance(node, ast.ClassDef) or node.name not in MODEL_CLASSES:
                continue
            for statement in node.body:
                if not isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    continue
                # Dunders are excluded on purpose: `super().__init__(...)` inside a
                # controller or a plain exception subclass is not a recordset call,
                # and matching on the name alone reports every one of them.
                if statement.name.startswith("__"):
                    continue
                # A property is read, never called, so its parameter list says nothing
                # about any call site.
                if any(
                    is_property(decorator) for decorator in statement.decorator_list
                ):
                    continue
                # The rule drops the leading receiver when it binds a call, so a method
                # that does not take one -- a `staticmethod` -- would come out shifted by
                # one parameter. Odoo's model classes have none today; this keeps a future
                # one from silently skewing every call site.
                first = (statement.args.posonlyargs or statement.args.args or [None])[0]
                if first is None or first.arg not in ("self", "cls"):
                    continue
                methods.setdefault(statement.name, statement.args)
    return methods


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--odoo",
        type=pathlib.Path,
        required=True,
        help="path to an odoo/odoo checkout with the release refs fetched",
    )
    parser.add_argument(
        "--output",
        type=pathlib.Path,
        default=pathlib.Path(__file__).parent.parent
        / "crates/ruff_linter/resources/odoo",
        help="directory the stubs are written to",
    )
    arguments = parser.parse_args()

    arguments.output.mkdir(parents=True, exist_ok=True)
    for version, ref, paths in VERSIONS:
        sha = subprocess.run(
            ["git", "rev-parse", ref],
            capture_output=True,
            text=True,
            cwd=arguments.odoo,
            check=False,
        ).stdout.strip()
        if not sha:
            print(f"{ref} is not available in {arguments.odoo}", file=sys.stderr)
            return 1
        methods = collect(arguments.odoo, ref, paths)
        if not methods:
            print(f"no model methods found at {ref}", file=sys.stderr)
            return 1
        lines = [
            f"# Odoo {version} ORM model method signatures, read from"
            f" {' and '.join(paths)}",
            f"# at {ref} ({sha}).",
            "#",
            "# Generated by scripts/generate_odoo_model_stubs.py -- do not edit by hand.",
            "# Annotations and default values are dropped; `=...` marks a parameter as",
            "# optional. Dunders are excluded, see the script for why.",
            "",
            "class BaseModel:",
        ]
        lines += [
            f"    def {name}({render_parameters(args)}): ..."
            for name, args in sorted(methods.items())
        ]
        destination = arguments.output / f"models_{version.replace('.', '')}.py"
        destination.write_text("\n".join(lines) + "\n")
        print(f"{destination}: {len(methods)} methods from {ref}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
