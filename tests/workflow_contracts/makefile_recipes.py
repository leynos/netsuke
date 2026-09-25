"""Reading a Makefile's targets, recipes, and commands.

A gate is only as good as the recipe behind it, and a contract that
matched the whole file would find a flag in a comment, in a variable, or
in a neighbouring target and report a command the gate does not run. The
recipe is every tab-indented line following the target, which is what
`make` runs and nothing else.

Separated from ``workflow_loading``, which owns the paths and the YAML
boundary, so neither module outgrows the 400-line limit the Python lint
gate enforces.

This module owns reading the Makefile. Every contract that wants its text —
whole, one target's prerequisites or recipe, or a named command variable —
comes through :func:`load_makefile`, so the two ways a read can fail are
diagnosed in one place instead of beside each assertion that happens to
need the file.
"""

import re
import shlex

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


def makefile_variable(name: str) -> str:
    """Return the default value a ``NAME ?=`` assignment gives in the Makefile.

    Reads the ``?=`` form alone: that is the shape a toolchain pin takes, and
    the ``=` form some other variables use is not a default the environment
    may displace. A pin written as ``=`` would otherwise be reported here as
    absent rather than as the wrong shape, sending a reader to the wrong line.

    Parameters
    ----------
    name : str
        The variable whose ``?=`` assignment is wanted.

    Returns
    -------
    str
        The assignment's value.
    """
    text = load_makefile()
    pattern = re.compile(rf"^{re.escape(name)} \?= (\S+)$", flags=re.MULTILINE)
    matches = pattern.findall(text)
    assert len(matches) == 1, (
        f"expected exactly one '{name} ?=' assignment in the Makefile, "
        f"found {len(matches)}"
    )
    return matches[0]


def makefile_target(target: str) -> tuple[list[str], str]:
    """Return one Make target's prerequisites and complete recipe text.

    The prerequisites are the words between the colon and the recipe, which
    is where a dependency is declared; the recipe is the tab-indented block
    that follows. Both are read from the target's own definition, so a
    prerequisite named in a comment or in a neighbouring target is not
    reported as this one's.

    Parameters
    ----------
    target : str
        The target whose definition is wanted.

    Returns
    -------
    tuple[list[str], str]
        The target's prerequisite words, and its recipe text.
    """
    text = load_makefile()
    match = re.search(
        rf"^{re.escape(target)}:([^\n#]*)(?:\s+##[^\n]*)?\n((?:\t[^\n]*\n?)*)",
        text,
        flags=re.MULTILINE,
    )
    assert match is not None, f"the Makefile must define the {target} target"
    prerequisites = match.group(1).split()
    return prerequisites, match.group(2)


def makefile_command(name: str) -> list[str]:
    """Return one continued Makefile command assignment as shell tokens.

    The value is joined across its backslash continuations before splitting,
    because the line breaks are Make's, not the command's: a flag that falls
    after a continuation is passed to the tool, and reading the lines
    separately would report it as absent. Shell quoting is preserved by
    :func:`shlex.split`, so ``--from 'pkg==1'`` reads as one token.

    Parameters
    ----------
    name : str
        The command variable whose value is wanted.

    Returns
    -------
    list[str]
        The command's shell tokens, continuations joined.
    """
    text = load_makefile()
    match = re.search(
        rf"^{re.escape(name)} = ((?:[^\n]*\\\n)*[^\n]+)$",
        text,
        flags=re.MULTILINE,
    )
    assert match is not None, f"the Makefile must define the {name} command"
    command = match.group(1).replace("\\\n", " ")
    return shlex.split(command)
