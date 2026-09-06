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

Alongside each stub it writes a `removed_<version>.py`: the model methods an
earlier supported version defined and that this version no longer defines on any
class at all. That set is what `removed-odoo-method-call` reports on, and it
cannot be read off the stubs alone -- a method that merely *moved*, like
`_condition_to_sql` becoming a `Field` method in 19.0, is gone from the model
classes while `field._condition_to_sql(...)` stays a correct call. Subtracting
every other class Odoo ships keeps that distinction out of the rule.

Both are generated artifacts: regenerate them, do not hand-edit them.

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
import tarfile
from typing import NamedTuple


class Version(NamedTuple):
    """One shipped Odoo version and where to read its ORM from."""

    name: str
    ref: str
    #: The modules defining the ORM model classes -- where the signatures come from.
    model_modules: list[str]
    #: The roots of the tree, read for defined method names only.
    tree_roots: list[str]


# The whole of Odoo, read for method names only, to decide which removals are
# real. Everything, not just the ORM: `removed-odoo-method-call` matches on the
# method name with no constraint on the receiver, so any name Odoo still defines
# on some class is a name whose call sites cannot be judged from the name alone.
# Measured on Odoo 19.0's own 8650 files, narrowing this to the ORM package left
# three reports, all of them `refresh` -- a browser's and a credential's, both
# correct code -- and widening it to the tree removes exactly those.
TREE = ["odoo", "addons"]


# Every version the rule ships a stub for: the ref to read it from, the modules
# the ORM model classes live in at that ref, and the roots of the tree to read
# every other class's method names from.
VERSIONS = [
    Version("16.0", "odoo/16.0", ["odoo/models.py"], TREE),
    Version("17.0", "odoo/17.0", ["odoo/models.py"], TREE),
    Version("18.0", "odoo/18.0", ["odoo/models.py"], TREE),
    Version(
        "19.0",
        "odoo/19.0",
        ["odoo/orm/models.py", "odoo/orm/models_transient.py"],
        TREE,
    ),
    Version(
        "20.0",
        "odoo/master",
        ["odoo/orm/models.py", "odoo/orm/models_transient.py"],
        TREE,
    ),
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


def is_test_path(path: str) -> bool:
    """Whether `path` is test code rather than something an addon really exposes.

    Odoo's test suites define throwaway models and classes, and `test_orm` keeps
    a `name_get` alive years after the ORM dropped it. A name that only survives
    in a test is not a name any call site could be reaching, so it must not keep
    a real removal out of the set.
    """
    return any(
        part == "tests" or part.startswith("test_")
        for part in pathlib.PurePosixPath(path).parts
    )


def collect_method_names(repo: pathlib.Path, ref: str, roots: list[str]) -> set[str]:
    """Every name defined as a method of some class under `roots` at `ref`.

    Unlike `collect`, this looks at all classes and keeps nothing but the names:
    it answers "could `something.<name>(...)` still be reaching Odoo code?", for
    which the parameter list is beside the point.

    Read through a single `git archive` rather than a `git show` per file, since
    at this width that is thousands of files per version.
    """
    names: set[str] = set()
    process = subprocess.Popen(
        ["git", "archive", "--format=tar", ref, "--", *roots],
        stdout=subprocess.PIPE,
        cwd=repo,
    )
    assert process.stdout is not None
    with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
        for member in archive:
            if not member.isfile() or not member.name.endswith(".py"):
                continue
            if is_test_path(member.name):
                continue
            content = archive.extractfile(member)
            if content is None:
                continue
            try:
                tree = ast.parse(content.read())
            except SyntaxError:
                # Odoo ships a few files that are not valid on the running
                # interpreter; a name defined in one is not worth a failed run.
                continue
            for node in ast.walk(tree):
                if not isinstance(node, ast.ClassDef):
                    continue
                names.update(
                    statement.name
                    for statement in node.body
                    if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef))
                )
    process.wait()
    return names


def removed_methods(
    per_version: dict[str, set[str]], surviving: set[str], version: Version
) -> dict[str, str]:
    """The methods gone by `version`, each mapped to the version that dropped it.

    A method counts as removed once it is absent from the model classes *and*
    from every other ORM class at `version`: `_setup_fields` became a module
    level function in 19.0, which does not make `self._setup_fields()` work, but
    `_condition_to_sql` became a `Field` method, which does keep
    `field._condition_to_sql(...)` correct.

    The comparison reaches back through every earlier shipped version rather than
    just the previous one, so a method dropped in 17.0 is still reported when the
    project is on 19.0.
    """
    order = [each.name for each in VERSIONS]
    earlier = order[: order.index(version.name)]
    removed: dict[str, str] = {}
    for index, name in enumerate(earlier):
        for method in per_version[name]:
            if method in per_version[version.name] or method in surviving:
                continue
            # Absent from the next shipped version, so that is the one that
            # dropped it -- and the last writer wins, which keeps a method that
            # came back and went again pinned to its final removal.
            successor = order[index + 1]
            if method not in per_version[successor]:
                removed[method] = successor
    return removed


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
    shas: dict[str, str] = {}
    signatures: dict[str, dict[str, ast.arguments]] = {}
    for version in VERSIONS:
        sha = subprocess.run(
            ["git", "rev-parse", version.ref],
            capture_output=True,
            text=True,
            cwd=arguments.odoo,
            check=False,
        ).stdout.strip()
        if not sha:
            print(
                f"{version.ref} is not available in {arguments.odoo}", file=sys.stderr
            )
            return 1
        methods = collect(arguments.odoo, version.ref, version.model_modules)
        if not methods:
            print(f"no model methods found at {version.ref}", file=sys.stderr)
            return 1
        shas[version.name] = sha
        signatures[version.name] = methods
        lines = [
            f"# Odoo {version.name} ORM model method signatures, read from"
            f" {' and '.join(version.model_modules)}",
            f"# at {version.ref} ({sha}).",
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
        destination = arguments.output / f"models_{version.name.replace('.', '')}.py"
        destination.write_text("\n".join(lines) + "\n")
        print(
            f"{destination}: {len(methods)} methods from {version.ref}", file=sys.stderr
        )

    # The removal diff needs every version's model methods, so it only runs once
    # they have all been read.
    names = {name: set(methods) for name, methods in signatures.items()}
    for version in VERSIONS:
        surviving = collect_method_names(
            arguments.odoo, version.ref, version.tree_roots
        )
        removed = removed_methods(names, surviving, version)
        lines = [
            f"# ORM model methods Odoo no longer has in {version.name}, each mapped to"
            " the version",
            "# that dropped it. Read from"
            f" {' and '.join(version.tree_roots)} at {version.ref}"
            f" ({shas[version.name]}),",
            "# alongside the model method sets of every earlier version listed here.",
            "#",
            "# Generated by scripts/generate_odoo_model_stubs.py -- do not edit by hand.",
            "# A method that only moved elsewhere in the ORM is not listed: it is still",
            "# callable, just on something other than a recordset.",
            "",
            "REMOVED = {",
        ]
        lines += [
            f'    "{method}": "{dropped_in}",'
            for method, dropped_in in sorted(removed.items())
        ]
        lines.append("}")
        destination = arguments.output / f"removed_{version.name.replace('.', '')}.py"
        destination.write_text("\n".join(lines) + "\n")
        print(
            f"{destination}: {len(removed)} methods removed by {version.name}",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
