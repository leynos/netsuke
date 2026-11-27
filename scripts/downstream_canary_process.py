"""Hold the downstream canary runner's process and host boundary.

``run_downstream_canary.py`` decides what a canary step does; this module is
the only place it starts a process or reads the host. Each launch goes through
a Cuprum catalogue that allows exactly the resolved executable, as
``docs/scripting-standards.md`` requires, and the inherited environment and
the executable lookup reach the runner as a :class:`Host`, so its decisions
can be tested with injected values rather than the test process's own.

Ownership: the canary runner is the only caller. The release-candidate
resolver has its own boundary, ``.github/scripts/release_candidate_git.py``,
because workflow scripts must load without third-party packages.
"""

import os
import shutil
import sys
import typing as typ

from cuprum import ExecutionContext, Program, ProgramCatalogue, SafeCmd, sh
from cuprum.catalogue import ProjectSettings

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path


class Which(typ.Protocol):
    """Look an executable up on a search path, as :func:`shutil.which` does.

    ``path`` is keyword-only because :func:`shutil.which`'s second positional
    parameter is the access ``mode``.
    """

    def __call__(self, name: str, /, *, path: str | None = None) -> str | None:
        """Return the executable's path, or ``None`` when it is not found."""
        ...


class Host(typ.NamedTuple):
    """The ambient inputs a canary step reads: its environment and ``PATH``.

    Examples
    --------
    >>> host = Host({"PATH": "/opt/bin"}, lambda name, *, path=None: None)
    >>> host.environment["PATH"]
    '/opt/bin'
    """

    environment: cabc.Mapping[str, str]
    which: Which

    @classmethod
    def ambient(cls) -> Host:
        """Return the running process's own environment and executable lookup.

        This is the one read of the process environment; ``main`` calls it
        and passes the result down.

        Returns
        -------
        Host
            A snapshot of ``os.environ`` and :func:`shutil.which`.
        """
        return cls(dict(os.environ), shutil.which)


def allowlisted(program: str, *arguments: str) -> SafeCmd:
    """Build a Cuprum command whose catalogue allows exactly ``program``.

    The canary's executables are resolved at run time -- the candidate
    ``netsuke`` and the selected ``ninja`` are absolute paths -- so a static
    catalogue cannot name them; one built per launch holds only the resolved
    path, and Cuprum refuses anything else.

    Returns
    -------
    SafeCmd
        The command, ready to run.

    Examples
    --------
    >>> allowlisted("/usr/bin/git", "--version").argv
    ('--version',)
    """
    catalogue = ProgramCatalogue(
        projects=(
            ProjectSettings(
                name="downstream-canary",
                programs=(Program(program),),
                documentation_locations=(),
                noise_rules=(),
            ),
        )
    )
    return sh.make(Program(program), catalogue=catalogue)(*arguments)


def run_tool(
    command: cabc.Sequence[str], workdir: Path, env: cabc.Mapping[str, str]
) -> bool:
    """Run one tool in the downstream checkout, streaming its output.

    The output is echoed rather than captured: it belongs in the job log,
    where a failed downstream gate's diagnostics are read, and never in the
    bounded provenance record. Cuprum discards a stream that is neither
    captured nor echoed.

    Parameters
    ----------
    command
        The program and its arguments.
    workdir
        The downstream checkout.
    env
        The complete child environment.

    Returns
    -------
    bool
        ``True`` when the tool exits successfully.
    """
    print(f"::group::{' '.join(command)}", flush=True)
    try:
        result = allowlisted(*command).run_sync(
            capture=False,
            echo=True,
            context=ExecutionContext(env=dict(env), cwd=workdir),
        )
    except OSError as error:
        print(f"cannot start {command[0]}: {error}", file=sys.stderr)
        return False
    finally:
        sys.stdout.flush()
        sys.stderr.flush()
        print("::endgroup::", flush=True)
    return result.exit_code == 0


def observed_head(workdir: Path, git: str | None) -> str | None:
    """Return the downstream checkout's ``HEAD`` commit, if Git can read it.

    Parameters
    ----------
    workdir
        The downstream checkout.
    git
        The resolved ``git`` executable, or ``None`` when there is none.

    Returns
    -------
    str | None
        The full commit identifier, or ``None`` when Git or the checkout is
        missing, or Git cannot read it.
    """
    if git is None or not workdir.is_dir():
        return None
    result = allowlisted(git, "-C", str(workdir), "rev-parse", "HEAD").run_sync()
    return (result.stdout or "").strip() if result.exit_code == 0 else None


__all__ = ["Host", "Which", "allowlisted", "observed_head", "run_tool"]
