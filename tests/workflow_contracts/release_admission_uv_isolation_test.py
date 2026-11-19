"""Prove the verifier's uv invocation ignores the candidate's configuration.

The admission gate runs from the workspace root, which is the candidate
checkout: content chosen by whoever called the reusable workflow. uv reads
`uv.toml` and `pyproject.toml` from the working directory before it starts a
script, so a candidate could use either file to redirect the package index
that supplies the gate's own pinned dependencies.

`UV_ISOLATION_FLAGS` refuses both files, and the source-pin contract asserts
the flags are present. Presence is not effect: a command whose flags were
reordered, or lost to a uv release that changed what they mean, would still
satisfy a text comparison. These cases run the workflow's own command against
a candidate laid out to exploit each file, so the property is observed rather
than read.

A wheel is built by hand and served offline to prove the run still installs
the script's own PEP 723 dependency, and the variables that alias the flags
are supplied to prove they cannot rescue an unflagged command.

Run via ``make test-workflow-contracts``.
"""

import base64
import hashlib
import os
import shlex
import subprocess  # ruff: ignore[suspicious-subprocess-import] - uv and the probe script are the boundary under test.
import typing as typ
import zipfile

import pytest
from release_admission_source_pin import (
    ADMISSION_JOB,
    verifier_gate_command,
)
from workflow_loading import RELEASE_WORKFLOW_PATH, job_steps, load_workflow

if typ.TYPE_CHECKING:
    from pathlib import Path

#: Printed by the probe the gate step is pointed at, so a run that reached the
#: script is distinguishable from one uv refused before starting it.
PROBE_SENTINEL = "verifier-probe-ran"

#: The identity the handcrafted wheel's module reports. It is a value no index
#: could serve, so printing it proves the dependency came from the local file
#: this case built rather than from anywhere else.
PROBE_IDENTITY = "local-wheel"

#: The dependency the offline probe declares, and the module it installs. The
#: name carries no hyphen because uv rejected the handcrafted file name with
#: one -- `admission-probe-dependency-0.1.0-py3-none-any.whl` was refused with
#: "Python tag must not start with a digit" -- and this case is about
#: resolution, not about uv's file-name parsing.
DEPENDENCY_NAME = "admissionprobe"
DEPENDENCY_MODULE = "admissionprobe"
DEPENDENCY_VERSION = "0.1.0"

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

#: Variables that would hand uv a configuration file, a project, or the flags'
#: own meanings from outside the case. An exported `UV_NO_CONFIG` would make
#: the `--no-config` control pass without the flag ever being refused, and an
#: exported `UV_FIND_LINKS` would let the wheel case resolve the probe's
#: dependency from somewhere this case did not build.
STRIPPED_VARIABLES = (
    "UV_CONFIG_FILE",
    "UV_PROJECT",
    "UV_NO_CONFIG",
    "UV_NO_PROJECT",
    "UV_OFFLINE",
    "UV_FIND_LINKS",
    "UV_CACHE_DIR",
    "UV_INDEX_URL",
    "UV_DEFAULT_INDEX",
    "UV_EXTRA_INDEX_URL",
)

#: The plain probe: a script that only prints the sentinel. It declares no
#: dependency, so uv runs it in the project environment when project discovery
#: is allowed, which is what makes the project trap observable.
PLAIN_PROBE = f'print("{PROBE_SENTINEL}")\n'


def _gate_command() -> str:
    """Return the uv invocation the release workflow actually declares."""
    return verifier_gate_command(
        job_steps(load_workflow(RELEASE_WORKFLOW_PATH), ADMISSION_JOB)
    )


def _script_path(command: str) -> str:
    """Return the workspace-relative script the command runs."""
    parts = shlex.split(command)
    return parts[parts.index("--script") + 1]


def _write_wheel(directory: Path) -> Path:
    """Build a wheel for the probe's declared dependency, with no build tool.

    A real index would make the case depend on the network and on a package
    nobody pins. The wheel is written directly: one module, the metadata that
    names it, the wheel marker, and a `RECORD` whose digests match the bytes
    written, which is what an installer checks.

    Parameters
    ----------
    directory
        Directory to write the wheel into.

    Returns
    -------
    Path
        The wheel file the run is told to resolve from.
    """
    directory.mkdir(parents=True, exist_ok=True)
    stem = f"{DEPENDENCY_NAME}-{DEPENDENCY_VERSION}"
    files = {
        f"{DEPENDENCY_MODULE}/__init__.py": f'IDENTITY = "{PROBE_IDENTITY}"\n',
        f"{stem}.dist-info/METADATA": (
            f"Metadata-Version: 2.1\nName: {DEPENDENCY_NAME}\n"
            f"Version: {DEPENDENCY_VERSION}\nSummary: verifier probe\n"
            "Requires-Python: >=3.8\n"
        ),
        f"{stem}.dist-info/WHEEL": (
            "Wheel-Version: 1.0\nGenerator: handcrafted\n"
            "Root-Is-Purelib: true\nTag: py3-none-any\n"
        ),
    }
    files[f"{stem}.dist-info/RECORD"] = _record(files)
    wheel = directory / f"{stem}-py3-none-any.whl"
    with zipfile.ZipFile(wheel, "w") as archive:
        for path, text in files.items():
            archive.writestr(path, text)
    return wheel


def _record(files: dict[str, str]) -> str:
    """Return a `RECORD` file naming every written member and its digest.

    Parameters
    ----------
    files
        The members written so far, excluding the record itself.

    Returns
    -------
    str
        The record's contents, with the record's own line left unhashed.
    """
    lines = []
    for path, text in files.items():
        encoded = text.encode("utf-8")
        digest = base64.urlsafe_b64encode(hashlib.sha256(encoded).digest())
        checksum = digest.rstrip(b"=").decode("ascii")
        lines.append(f"{path},sha256={checksum},{len(encoded)}")
    lines.append(f"{DEPENDENCY_NAME}-{DEPENDENCY_VERSION}.dist-info/RECORD,,")
    return "\n".join(lines) + "\n"


def _dependency_probe() -> str:
    """Return a script whose PEP 723 block declares the local dependency.

    The block is what the gate's real script uses for its own pinned
    dependencies, so this exercises the same resolution path.

    Returns
    -------
    str
        The script the command under test runs.
    """
    return (
        "# /// script\n"
        f'# dependencies = ["{DEPENDENCY_NAME}=={DEPENDENCY_VERSION}"]\n'
        "# ///\n"
        f"import {DEPENDENCY_MODULE}\n"
        f'print("{PROBE_SENTINEL}", {DEPENDENCY_MODULE}.IDENTITY)\n'
    )


def _probe_workspace(root: Path, script_path: str, *, probe: str = PLAIN_PROBE) -> Path:
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
    probe
        The script body to write at that path.

    Returns
    -------
    Path
        ``root``, holding the traps and the probe.
    """
    script = root / script_path
    script.parent.mkdir(parents=True)
    script.write_text(probe, encoding="utf-8")
    (root / "uv.toml").write_text(CONFIG_TRAP, encoding="utf-8")
    (root / "pyproject.toml").write_text(PROJECT_TRAP, encoding="utf-8")
    return root


def _run(
    command: str,
    workspace: Path,
    *,
    inherited: dict[str, str] | None = None,
    extra_environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run one gate command from ``workspace``, with the aliases stripped.

    ``inherited`` stands in for the caller's exported variables and goes
    through the filter with the real ones, so a case can show what the filter
    does with a variable it was given. ``extra_environment`` is applied after
    the filter, for values the case supplies deliberately.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished gate command, with its captured streams and exit status.
    """
    source = {**os.environ, **(inherited or {})}
    environment = {
        name: value for name, value in source.items() if name not in STRIPPED_VARIABLES
    }
    environment.update(extra_environment or {})
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


def test_the_gate_command_installs_the_scripts_pinned_dependency(
    tmp_path: Path,
) -> None:
    """Resolve the probe's declared dependency offline from a local wheel.

    The candidate's traps are still in place, so the sentinel can only be
    printed by a run that refused both files and then resolved the script's own
    PEP 723 dependency. The identity proves the installed module is the wheel
    this case built.
    """
    command = _gate_command()
    wheel = _write_wheel(tmp_path / "dist")
    workspace = _probe_workspace(
        tmp_path / "workspace", _script_path(command), probe=_dependency_probe()
    )
    result = _run(
        command,
        workspace,
        extra_environment={
            # `--find-links` takes a directory or a links page, not a wheel, so
            # the case points at the directory it filled.
            "UV_FIND_LINKS": str(wheel.parent),
            "UV_OFFLINE": "1",
            # A cache of this case's own, so a wheel already cached elsewhere on
            # the host cannot stand in for the one this case built.
            "UV_CACHE_DIR": str(tmp_path / "cache"),
        },
    )
    assert result.returncode == 0, (
        "the gate command must resolve the script's own dependency while "
        f"refusing the candidate's files: exited {result.returncode}: "
        f"{result.stderr}"
    )
    assert f"{PROBE_SENTINEL} {PROBE_IDENTITY}" in result.stdout, (
        f"the installed module must be the wheel this case built, got {result.stdout!r}"
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


@pytest.mark.parametrize(
    ("variable", "flag"),
    [
        ("UV_NO_CONFIG", "--no-config"),
        ("UV_NO_PROJECT", "--no-project"),
    ],
    ids=["config", "project"],
)
def test_an_inherited_variable_cannot_rescue_an_unflagged_command(
    tmp_path: Path,
    variable: str,
    flag: str,
) -> None:
    """Require the inherited environment to be stripped of the flags' aliases.

    uv reads `UV_NO_CONFIG` and `UV_NO_PROJECT` as the flags themselves. If the
    case inherited either one, the matching negative control would run isolated
    anyway and pass without ever showing the flag changes uv's behaviour. The
    variable is handed to :func:`_run` as an inherited value and the command is
    stripped of the flag, so the filter is what decides: a run that honours the
    variable succeeds and this assertion fails.
    """
    command = _without_flag(_gate_command(), flag)
    workspace = _probe_workspace(tmp_path, _script_path(command))
    result = _run(command, workspace, inherited={variable: "1"})
    assert result.returncode != 0, (
        f"the inherited {variable} must not reach uv: without {flag} the "
        f"candidate's trap must still stop the run, but it exited 0: "
        f"{result.stdout}"
    )
    assert PROBE_SENTINEL not in result.stdout, (
        f"a run controlled by an inherited {variable} must not reach the script"
    )
