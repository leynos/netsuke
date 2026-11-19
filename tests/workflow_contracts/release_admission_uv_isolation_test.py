"""Prove the verifier's uv invocation ignores the candidate's configuration.

The admission gate runs from the workflow's workspace root, which is the
candidate checkout: content chosen by whoever called the reusable workflow.
uv reads `uv.toml` and `pyproject.toml` from the working directory before it
starts a script, so a candidate could use either file to redirect the package
index that supplies the gate's own pinned dependencies, or to make the run
resolve an interpreter of its choosing.

:data:`release_admission_source_pin.UV_ISOLATION_FLAGS` refuses both files, and
the source-pin contract asserts the flags are present. Presence is not effect:
a command whose flags were reordered, or lost to a uv release that changed what
they mean, would still satisfy a text comparison. These cases run the
workflow's own command against a candidate laid out to exploit each file, so
the property is observed rather than read.

Run via ``make test-workflow-contracts``.
"""

import os
import shlex
import subprocess  # ruff: ignore[suspicious-subprocess-import] - uv and the probe script are the boundary under test.
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

from release_admission_source_pin import (
    ADMISSION_JOB,
    verifier_gate_command,
)
from workflow_loading import RELEASE_WORKFLOW_PATH, job_steps, load_workflow

#: Printed by the probe the gate step is pointed at, so a run that reached the
#: script is distinguishable from one uv refused before starting it.
PROBE_SENTINEL = "verifier-probe-ran"

#: A `uv.toml` that no uv release accepts. `index-url` must be a string, so a
#: run that reads this file stops before it reaches the script. The trap has to
#: be unreadable rather than merely different: a candidate index that answered
#: normally would leave a successful run ambiguous about which index served it.
CONFIG_TRAP = "index-url = 123\n"

#: A project file whose interpreter requirement cannot be satisfied. Project
#: discovery would honour it, so this is the same argument for the second flag.
PROJECT_TRAP = (
    '[project]\nname = "candidate"\nversion = "0.1.0"\nrequires-python = ">=99"\n'
)

#: Variables that would hand uv a configuration file or project from outside
#: the case. They are removed so the run answers for the flags in the command
#: rather than for the reviewer's shell.
STRIPPED_VARIABLES = ("UV_CONFIG_FILE", "UV_PROJECT")


def _gate_command() -> str:
    """Return the gate command the release workflow actually declares.

    Returns
    -------
    str
        The uv invocation naming the verifier's copy of the entry point.
    """
    return verifier_gate_command(
        job_steps(load_workflow(RELEASE_WORKFLOW_PATH), ADMISSION_JOB)
    )


def _script_path(command: str) -> str:
    """Return the script the command runs, as a workspace-relative path.

    Parameters
    ----------
    command
        A gate command in the workflow's own spelling.

    Returns
    -------
    str
        The argument that follows ``--script``.
    """
    parts = shlex.split(command)
    return parts[parts.index("--script") + 1]


def _probe_workspace(root: Path, script_path: str) -> Path:
    """Lay out a candidate checkout that tries to redirect the run.

    Both traps sit at the workspace root, which is where uv looks for them
    before it resolves the script's own environment. A run that reaches the
    probe therefore ignored both files.

    Parameters
    ----------
    root
        Temporary directory standing in for the workflow's workspace.
    script_path
        The workspace-relative path the command runs.

    Returns
    -------
    Path
        ``root``, holding the traps and the probe.
    """
    script = root / script_path
    script.parent.mkdir(parents=True)
    script.write_text(f'print("{PROBE_SENTINEL}")\n', encoding="utf-8")
    (root / "uv.toml").write_text(CONFIG_TRAP, encoding="utf-8")
    (root / "pyproject.toml").write_text(PROJECT_TRAP, encoding="utf-8")
    return root


def _run(command: str, workspace: Path) -> subprocess.CompletedProcess[str]:
    """Run one gate command with ``workspace`` as its working directory.

    Parameters
    ----------
    command
        The command under test, in the workflow's own spelling.
    workspace
        Directory the command runs from, holding the traps and the probe.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished run, with its streams captured.
    """
    environment = {
        name: value
        for name, value in os.environ.items()
        if name not in STRIPPED_VARIABLES
    }
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        shlex.split(command),
        cwd=workspace,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=180,
    )


def _without_flag(command: str, flag: str) -> str:
    """Return ``command`` with one isolation flag removed.

    Parameters
    ----------
    command
        A gate command that carries the flag.
    flag
        The flag to drop, matched as a whole token.

    Returns
    -------
    str
        The command a run without that flag would use.
    """
    parts = [part for part in shlex.split(command) if part != flag]
    assert len(parts) == len(shlex.split(command)) - 1, (
        f"the gate command must carry {flag!r}"
    )
    return shlex.join(parts)


def test_the_gate_command_ignores_a_hostile_candidate_checkout(
    tmp_path: Path,
) -> None:
    """Run the declared command over a candidate that tries to redirect it."""
    command = _gate_command()
    workspace = _probe_workspace(tmp_path, _script_path(command))
    result = _run(command, workspace)
    assert result.returncode == 0, (
        "the gate command must ignore the candidate's uv configuration: "
        f"{command!r} exited {result.returncode}: {result.stderr}"
    )
    assert PROBE_SENTINEL in result.stdout, (
        f"the gate command must reach the probe script, got {result.stdout!r}"
    )


def test_the_configuration_trap_catches_a_command_without_no_config(
    tmp_path: Path,
) -> None:
    """Show the trap fires when ``--no-config`` is dropped."""
    command = _without_flag(_gate_command(), "--no-config")
    workspace = _probe_workspace(tmp_path, _script_path(command))
    result = _run(command, workspace)
    assert result.returncode != 0, (
        "without --no-config the candidate's uv.toml must stop the run, but "
        f"{command!r} exited 0: {result.stdout}"
    )
    assert PROBE_SENTINEL not in result.stdout, (
        "a run that read the candidate's uv.toml must not reach the script"
    )
    assert "uv.toml" in result.stderr, (
        f"the refusal must name the file that caused it, got {result.stderr!r}"
    )


def test_the_project_trap_catches_a_command_without_no_project(
    tmp_path: Path,
) -> None:
    """Show the trap fires when ``--no-project`` is dropped."""
    command = _without_flag(_gate_command(), "--no-project")
    workspace = _probe_workspace(tmp_path, _script_path(command))
    result = _run(command, workspace)
    assert result.returncode != 0, (
        "without --no-project the candidate's interpreter requirement must "
        f"stop the run, but {command!r} exited 0: {result.stdout}"
    )
    assert PROBE_SENTINEL not in result.stdout, (
        "a run that honoured the candidate's project must not reach the script"
    )
