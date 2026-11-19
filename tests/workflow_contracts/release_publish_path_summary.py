"""Require the authorization step's script to record every decision.

``release_publish_path_authorization`` proves the workflow reaches the decision
step unconditionally. This module proves the run reaches the record: the step's
own entry point must call the summary writer without passing through a branch,
a loop, or a handler, so a decision cannot be made and then left unrecorded.

The script is read as a syntax tree rather than as text. A textual search would
accept a call that sits inside an ``if``, which is exactly the shape that lets
an authorization decision escape its record. The tree is read as strictly as
the text would be read loosely: only a statement that *is* the writer call
counts, so a call that an expression can short-circuit away is refused too.
"""

import ast
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import pathlib

ENTRY_POINT = "export_authorization"
SUMMARY_WRITER = "write_summary"


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
    """Return whether a statement is itself a call to ``name``.

    Only an expression statement whose whole value is the call counts. A call
    reached through ``ast.walk`` proves nothing: inside an ``if`` it may be
    skipped, and inside ``False and f()`` it is never evaluated at all, so a
    search that accepted it would report a record the run can omit.

    Returns
    -------
    bool
        Whether the statement is exactly a ``name`` call.
    """
    if not isinstance(statement, ast.Expr):
        return False
    call = statement.value
    return isinstance(call, ast.Call) and called_name(call) == name


def check_summary_record(
    violations: list[str], *, script: pathlib.Path | None = None
) -> None:
    """Require the entry point to call the summary writer unconditionally.

    The check reports a script that cannot be read or parsed, one that defines
    no entry point, and one whose entry point never writes the record directly.
    A writer call inside a branch, a loop, a handler, or a short-circuiting
    expression is not a direct write, so each of those shapes is reported.
    Every reported violation is prefixed with ``authorization.summary`` so a
    caller can tell it from the wiring checks.
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
