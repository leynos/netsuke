"""Reading what a Makefile target actually runs.

A gate is only as good as the recipe behind it, and a contract that
matched the whole file would find a flag in a comment, in a variable, or
in a neighbouring target and report a command the gate does not run. The
recipe is every tab-indented line following the target, which is what
`make` runs and nothing else.

Separated from ``workflow_loading``, which owns the paths and the YAML
boundary, so neither module outgrows the 400-line limit the Python lint
gate enforces.

This module owns reading the Makefile. Every contract that wants its text —
whole, or one target's recipe — comes through :func:`load_makefile`, so the
two ways a read can fail are diagnosed in one place instead of beside each
assertion that happens to need the file.
"""

import pytest
from workflow_loading import MAKEFILE_PATH


class MakefileTargetError(LookupError):
    """Raised when the Makefile declares no such target.

    Distinguished from a target whose recipe is empty. A contract
    asserting a flag would otherwise read a missing target and an
    unflagged one the same way, and report a renamed target as a
    changed command.
    """


def load_makefile() -> str:
    """Read the repository Makefile, failing the test when it cannot be read.

    A contract cannot say anything about a Makefile it never read, and the
    read is fallible in two ways that look nothing like each other from a
    caller several frames away: the file may be missing, or its bytes may not
    decode. Both are reported here, with the path, rather than surfacing as an
    opaque ``OSError`` beside an assertion about flags.

    Returns
    -------
    str
        The Makefile's contents.
    """
    # The assignment and the return are split deliberately. ``pytest.fail``
    # raises, but a `return` inside the `try` and a call inside the `except`
    # read to Pylint as a function that sometimes returns an expression and
    # sometimes does not. Reading first and returning once removes the
    # ambiguity rather than silencing the diagnostic.
    try:
        text = MAKEFILE_PATH.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        pytest.fail(f"could not read {MAKEFILE_PATH}: {error}")
    return text


def makefile_recipe(target: str) -> str:
    """Return the recipe lines of one Makefile target.

    The recipe is every tab-indented line following the target, which is
    what `make` runs. Reading the whole file instead would find a flag in
    a comment, in a variable, or in a neighbouring target, and report a
    command the gate does not run.

    Parameters
    ----------
    target : str
        The target whose recipe is wanted.

    Returns
    -------
    str
        The recipe's lines, joined by newlines.

    Raises
    ------
    MakefileTargetError
        If the Makefile declares no such target.
    """
    lines = load_makefile().splitlines()
    start = next(
        (
            index
            for index, line in enumerate(lines)
            if line.startswith(f"{target}:") or line.startswith(f"{target} ")
        ),
        None,
    )
    if start is None:
        message = f"the Makefile declares no {target!r} target"
        raise MakefileTargetError(message)
    recipe: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("\t"):
            recipe.append(line)
        elif line.strip():
            break
    return "\n".join(recipe)
