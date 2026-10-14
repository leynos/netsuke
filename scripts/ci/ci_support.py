"""Command and path helpers shared by the CI helper scripts.

The scripts replace inline shell in ``.github/workflows/ci.yml``. Each one is
a Cyclopts application that reads its parameters from ``INPUT_*`` environment
variables (and the ambient GitHub Actions variables it needs), runs external
programs through cuprum's allowlisted catalogue, and uses ``pathlib`` for the
filesystem. This module holds the command half they share: the catalogue, a
thin synchronous runner, and the two path operations a step needs -- publishing
a directory to later steps, and asking whether a cached payload is already in
place.

Downloading and extracting a release archive is the other half, and lives in
:mod:`ci_archive` so this module stays a single concern.

Every function here is exercised by ``scripts/tests/test_ci_support.py``.
"""

import dataclasses
import os
import stat
import typing as typ

from cuprum import (
    CommandResult,
    ExecutionContext,
    Program,
    ProgramCatalogue,
    ProjectSettings,
    sh,
)

if typ.TYPE_CHECKING:
    import pathlib

PROJECT_NAME = "netsuke-ci"
DOCUMENTATION = ("docs/developers-guide.md",)
#: Release archives come from GitHub over HTTPS; tests serve fixtures from
#: ``file://`` URLs so the real download path is exercised without a network.
ALLOWED_URL_SCHEMES = frozenset({"https", "file"})
DOWNLOAD_TIMEOUT_SECONDS = 120.0
EXECUTABLE_MODE = 0o755


class CiScriptError(RuntimeError):
    """An actionable failure whose message names the remediation."""


def catalogue(*programs: str) -> ProgramCatalogue:
    """Return a cuprum catalogue allowing exactly ``programs``.

    Parameters
    ----------
    programs
        Program names, or absolute paths for executables the script staged
        itself, that the calling script is permitted to run.

    Returns
    -------
    ProgramCatalogue
        A catalogue whose single project lists ``programs``.
    """
    return ProgramCatalogue(
        projects=(
            ProjectSettings(
                name=PROJECT_NAME,
                programs=tuple(Program(program) for program in programs),
                documentation_locations=DOCUMENTATION,
                noise_rules=(),
            ),
        )
    )


@dataclasses.dataclass(frozen=True, slots=True)
class RunSpec:
    """How one command invocation should behave, separate from what it runs.

    The program and its arguments are positional, so they read as the command
    being run; these are the two things about the *invocation* a caller
    sometimes needs to change. Grouping them keeps a call site from growing a
    parameter per concern, and a script that never changes one simply omits
    the group.
    """

    allowed: ProgramCatalogue
    echo: bool = True
    env: dict[str, str] | None = None


def run(program: str, *args: str, spec: RunSpec) -> CommandResult:
    """Run one allowlisted program synchronously and return its result.

    Parameters
    ----------
    program
        The program to run; it must be in ``spec.allowed``.
    args
        Arguments passed verbatim, never through a shell.
    spec
        The catalogue permitting ``program``, whether to mirror the child's
        output to this process's streams as well as capturing it, and any
        environment variables overlaid on the live environment for this run
        only.

    Returns
    -------
    CommandResult
        The exit code and captured output; a non-zero exit is returned, not
        raised, so callers decide what it means.
    """
    command = sh.make(Program(program), catalogue=spec.allowed)(*args)
    return command.run_sync(
        echo=spec.echo,
        context=ExecutionContext(env=spec.env),
    )


def describe_failure(result: CommandResult) -> str:
    """Return a one-line description of a failed command for an error message."""
    argv = " ".join((str(result.program), *result.argv))
    detail = (result.stderr or "").strip().splitlines()
    suffix = f": {detail[-1]}" if detail else ""
    return f"`{argv}` exited {result.exit_code}{suffix}"


def append_github_path(directory: pathlib.Path, github_path: pathlib.Path) -> None:
    """Publish ``directory`` to later workflow steps through ``GITHUB_PATH``.

    Parameters
    ----------
    directory
        The directory to prepend to ``PATH`` for the rest of the job.
    github_path
        The file GitHub Actions names in ``GITHUB_PATH``.
    """
    with github_path.open("a", encoding="utf-8") as handle:
        handle.write(f"{directory}\n")


def prepend_path(directory: pathlib.Path) -> None:
    """Prepend ``directory`` to this process's ``PATH`` for later commands."""
    current = os.environ.get("PATH", "")
    os.environ["PATH"] = (
        f"{directory}{os.pathsep}{current}" if current else str(directory)
    )


def is_executable_file(path: pathlib.Path) -> bool:
    """Return whether ``path`` is a regular file this process may execute.

    Absence is an ordinary answer, not a failure: a cache volume that has
    never been populated reports ``False`` so the caller installs the
    payload. A path that is present but cannot be inspected is a different
    thing, and is raised rather than folded into that ``False``, so an
    unreadable cache cannot be mistaken for an empty one and silently
    trigger a reinstall that would fail the same way.

    Returns
    -------
    bool
        ``True`` only for a regular file this process may execute.

    Raises
    ------
    CiScriptError
        If ``path`` exists but its status cannot be read.
    """
    try:
        status = path.stat()
    except FileNotFoundError, NotADirectoryError:
        return False
    except OSError as error:
        message = f"cannot inspect {path}: {error}"
        raise CiScriptError(message) from error
    return stat.S_ISREG(status.st_mode) and os.access(path, os.X_OK)
