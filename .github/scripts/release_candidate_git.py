"""Run Git for the release-candidate resolver through a Cuprum catalogue.

``resolve_release_candidate.py`` decides which commit the downstream canaries
build; this module is its only process boundary. It runs the ``git`` found on
``PATH``, in the current directory, through a catalogue that allows exactly
that executable, as ``docs/scripting-standards.md`` requires, and turns a
failing command into :class:`GitCommandError`, the one failure the resolver's
``GitRunner`` contract names.
"""

import shutil
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


class GitCommandError(RuntimeError):
    """Report a Git command that exited unsuccessfully."""


def run_git(arguments: cabc.Sequence[str]) -> str:
    """Run Git in the current directory and return its standard output.

    A failing Git command raises :class:`GitCommandError`.

    Parameters
    ----------
    arguments
        Git arguments, excluding the ``git`` program name.

    Returns
    -------
    str
        The command's standard output.

    Raises
    ------
    FileNotFoundError
        When no ``git`` executable is on ``PATH``.
    GitCommandError
        When Git exits unsuccessfully.
    """
    # Imported here, as ``release_admission.py`` does, because the
    # ``lint-workflow-scripts`` gate loads every module under
    # ``.github/scripts`` without its PEP 723 dependencies installed.
    from cuprum import Program, ProgramCatalogue, sh
    from cuprum.catalogue import ProjectSettings

    program = shutil.which("git")
    if program is None:
        msg = "git is not on PATH"
        raise FileNotFoundError(msg)
    catalogue = ProgramCatalogue(
        projects=(
            ProjectSettings(
                name="resolve-release-candidate",
                programs=(Program(program),),
                documentation_locations=(),
                noise_rules=(),
            ),
        )
    )
    result = sh.make(Program(program), catalogue=catalogue)(*arguments).run_sync()
    if result.exit_code != 0:
        msg = f"git {' '.join(arguments)} exited {result.exit_code}"
        raise GitCommandError(msg)
    return result.stdout or ""


__all__ = ("GitCommandError", "run_git")
