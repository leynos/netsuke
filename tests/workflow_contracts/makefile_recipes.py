"""Reading what a Makefile target actually runs.

A gate is only as good as the recipe behind it, and a contract that
matched the whole file would find a flag in a comment, in a variable, or
in a neighbouring target and report a command the gate does not run. The
recipe is every tab-indented line following the target, which is what
`make` runs and nothing else.

Separated from ``workflow_loading``, which owns the paths and the YAML
boundary, so neither module outgrows the 400-line limit the Python lint
gate enforces.
"""

from workflow_loading import MAKEFILE_PATH


class MakefileTargetError(LookupError):
    """Raised when the Makefile declares no such target.

    Distinguished from a target whose recipe is empty. A contract
    asserting a flag would otherwise read a missing target and an
    unflagged one the same way, and report a renamed target as a
    changed command.
    """


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
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
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
