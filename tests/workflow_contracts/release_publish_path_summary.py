"""Require the authorization step's script to record every decision.

``release_publish_path_authorization`` proves the workflow reaches the decision
step unconditionally. This module proves the run reaches the record: the step's
own entry point must call the summary writer without passing through a branch,
a loop, or a handler, so a decision cannot be made and then left unrecorded.

The script is read as a syntax tree rather than as text. A textual search would
accept a call that sits inside an ``if``, which is exactly the shape that lets
an authorization decision escape its record.
"""

import ast
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import pathlib

ENTRY_POINT = "export_authorization"
SUMMARY_WRITER = "write_summary"
#: Statements that can skip or repeat their body. A call inside one is not
#: evidence that every run reaches it.
_GUARDING_STATEMENTS = (ast.If, ast.Try, ast.While, ast.For, ast.AsyncFor)


def default_script() -> pathlib.Path:
    """Name the script the workflow's authorization step runs."""
    return REPO_ROOT / "scripts" / "authorize_release_writes.py"


def called_name(node: ast.Call) -> str | None:
    """Return the simple name a call invokes, or ``None`` for a dynamic target.

    Returns
    -------
    str | None
        The identifier or attribute name, when the call target has one.
    """
    match node.func:
        case ast.Name(id=name):
            return name
        case ast.Attribute(attr=name):
            return name
        case _:
            return None


def is_unconditional_call(statement: ast.stmt, name: str) -> bool:
    """Return whether a statement calls ``name`` outside a guarding body.

    Returns
    -------
    bool
        Whether the statement is not a guarding construct and calls ``name``.
    """
    if isinstance(statement, _GUARDING_STATEMENTS):
        return False
    return any(
        isinstance(node, ast.Call) and called_name(node) == name
        for node in ast.walk(statement)
    )


def check_summary_record(
    violations: list[str], *, script: pathlib.Path | None = None
) -> None:
    """Require the entry point to call the summary writer unconditionally.

    The check reports a script that cannot be read or parsed, one that defines
    no entry point, and one whose entry point reaches the writer only through a
    guarding construct. Every reported violation is prefixed with
    ``authorization.summary`` so a caller can tell it from the wiring checks.
    """
    target = default_script() if script is None else script
    try:
        tree = ast.parse(target.read_text(encoding="utf-8"), filename=str(target))
    except (OSError, SyntaxError) as error:
        violations.append(f"authorization.summary: cannot read {target.name}: {error}")
        return
    entry = next(
        (
            node
            for node in tree.body
            if isinstance(node, ast.FunctionDef) and node.name == ENTRY_POINT
        ),
        None,
    )
    if entry is None:
        violations.append(
            f"authorization.summary: {target.name} defines no {ENTRY_POINT}"
        )
        return
    if not any(
        is_unconditional_call(statement, SUMMARY_WRITER) for statement in entry.body
    ):
        violations.append(
            "authorization.summary: the decision record must be written unconditionally"
        )
