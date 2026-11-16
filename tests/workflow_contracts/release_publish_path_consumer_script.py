"""Read a smoke step's script as bash runs it, not as one line of it.

The reviewed invocation is one command inside a step's ``run`` block, so an
exact match on that line says nothing about the script around it. ``exit 0``
above it makes it unreachable, ``if false; then`` above it confines it to a
branch bash never takes, and ``set +e`` above it lets a refusal fall through to
an ``exit 0`` below. The readers here answer those questions, and only from the
executable lines, so a comment naming a construct cannot stand in for one.

Reachability and failure propagation are read as a shape rather than as a
search for one suspicious line. A step may prepare the invocation with
``set -…`` and plain assignments and nothing else; every other construct is
refused rather than assumed harmless, because a construct nobody reviewed is
exactly what a contract must not wave through.

Which command the step must run is asserted in
``release_publish_path_consumer_commands``, which composes these readers.

Run via ``make test-workflow-contracts``.
"""

import re
import shlex

from release_publish_path_consumer_shape import SMOKE_CLI, command_lines

#: The only lines a step may run before the reviewed invocation: an assignment
#: of one variable, or a ``set`` that enables options. Shell separators and
#: substitutions that run a command are excluded from an assignment's value, so
#: a line cannot both assign and run something.
_ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=[^;&|`]*")
_ENABLING_SET = re.compile(r"set -[A-Za-z]+(?: [A-Za-z][A-Za-z0-9_]*)*")


def ends_successfully(line: str) -> bool:
    """Return whether one shell command line exits the step successfully.

    Returns
    -------
    bool
        True for a bare ``exit`` or ``return``, or one naming status ``0``.
        A non-zero status fails the step instead of skipping the rest of it,
        so it makes no command unreachable.

    Examples
    --------
    >>> ends_successfully("exit 0"), ends_successfully("exit 1")
    (True, False)
    """
    try:
        tokens = tuple(shlex.split(line))
    except ValueError:
        return False
    return tokens[:1] in {("exit",), ("return",)} and tokens[1:] in {(), ("0",)}


def _is_prologue(line: str) -> bool:
    """Return whether one line only prepares the step for the invocation.

    Returns
    -------
    bool
        True for an assignment of one variable or a ``set`` that enables
        options, which are the two things a step needs before the command it
        exists to run. Everything else — a conditional, a loop, a bare command,
        or a ``set +e`` — can decide whether the invocation runs at all.
    """
    return (
        _ASSIGNMENT.fullmatch(line) is not None
        or _ENABLING_SET.fullmatch(line) is not None
    )


def unsupported_prologue(step: dict[str, object]) -> str | None:
    r"""Return the first line before the CLI that could skip or disarm it.

    The reviewed invocation is one command inside a script, so an exact match
    on that line says nothing about the script around it. ``exit 0`` above it
    makes it unreachable, ``if false; then`` above it confines it to a branch
    bash never takes, and ``set +e`` above it lets a refusal fall through to an
    ``exit 0`` below. Only a prologue that plainly prepares the step is
    accepted, so a construct nobody reviewed is refused rather than assumed
    harmless.

    Returns
    -------
    str | None
        The first line ahead of :data:`SMOKE_CLI` that is not a plain prologue,
        or ``None`` when every line ahead of the invocation prepares the step.

    Examples
    --------
    >>> step = {"run": "set -e\nexit 0\nuv run x scripts/consumer_smoke_cli.py"}
    >>> unsupported_prologue(step)
    'exit 0'
    """
    for line in command_lines(step.get("run")):
        if SMOKE_CLI in line:
            return None
        if not _is_prologue(line):
            return line
    return None


def ends_after_smoke_cli(step: dict[str, object]) -> str | None:
    """Return the line that ends a step successfully after the CLI runs.

    A success exit below the invocation absorbs the refusal: with the shell's
    fail-fast disabled, or never enabled, a verifier that exits non-zero is
    followed by ``exit 0`` and the step reports success.

    Returns
    -------
    str | None
        The first success exit that follows :data:`SMOKE_CLI`, or ``None`` when
        nothing after the invocation would end the step successfully.
    """
    lines = command_lines(step.get("run"))
    position = next(
        (index for index, line in enumerate(lines) if SMOKE_CLI in line), None
    )
    if position is None:
        return None
    return next(
        (line for line in lines[position + 1 :] if ends_successfully(line)), None
    )


__all__ = ["ends_after_smoke_cli", "ends_successfully", "unsupported_prologue"]
