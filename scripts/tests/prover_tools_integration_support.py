"""Fixtures and bounded command runners for the prover-tools integration tests.

The tests drive the pinned ``rust-prover-tools`` CLI and the repository's
``make verus`` target against a fake verifier, so they need a temporary
verifier, proof files, a single-word stand-in for the multi-word
``PROVER_TOOLS`` command, and a runner that cannot hang the suite.
"""

import json
import os
import pathlib
import shlex
import shutil
import signal
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - test CLI boundary.
import sys
import textwrap

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]

# A stalled fetch or CLI process would otherwise hang the suite until the CI job
# times out, with no indication of which command stalled.
COMMAND_TIMEOUT_SECONDS = 180


class CommandTimeoutError(RuntimeError):
    """Report a command that outlived the deadline and had its group killed.

    The command and whatever output it had produced are carried in the message
    so a CI failure names the stalled invocation rather than only the deadline.
    """

    def __init__(
        self,
        command: list[str],
        stdout: str,
        stderr: str,
        timeout_seconds: int = COMMAND_TIMEOUT_SECONDS,
    ) -> None:
        """Initialise the error from the stalled command and its output.

        Parameters
        ----------
        command
            The argv that outlived the deadline.
        stdout
            Output captured before the process group was killed.
        stderr
            Error output captured before the process group was killed.
        timeout_seconds
            The deadline the command exceeded.
        """
        super().__init__(
            f"command exceeded {timeout_seconds}s and was killed: "
            f"{shlex.join(command)}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        )


def run_command(
    command: list[str],
    *,
    cwd: pathlib.Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run ``command`` under a deadline and kill its whole process group.

    ``subprocess.run(timeout=...)`` kills only the direct child, leaving
    descendants that hold the captured pipes open so the read never returns.
    Starting the child in its own session and killing the group on expiry avoids
    that. The completed process mirrors ``subprocess.run``'s result so callers
    treat both paths identically.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished command, with output captured.

    Raises
    ------
    CommandTimeoutError
        If the command outlives ``COMMAND_TIMEOUT_SECONDS``. Its process group
        is killed before the error is raised.
    """
    # The context manager's exit waits for the child, which is safe here because
    # both paths below reap it first: ``communicate`` returns only once the child
    # has exited, and the timeout path kills the group before reaping.
    with subprocess.Popen(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        shell=False,
        start_new_session=True,
    ) as process:
        try:
            stdout, stderr = process.communicate(timeout=COMMAND_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
            raise CommandTimeoutError(command, stdout, stderr) from None
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def write_launcher(executable: pathlib.Path, body: str) -> None:
    """Write ``body`` as a Python script and a launcher that runs it.

    A ``#!<interpreter>`` line cannot carry an interpreter path containing
    spaces, so the interpreter and the script are passed to ``/bin/sh`` as two
    separate, quoted arguments instead.
    """
    source = executable.with_name(f"{executable.name}.py")
    source.write_text(textwrap.dedent(body), encoding="utf-8")
    executable.write_text(
        f"#!/bin/sh\nexec {shlex.quote(sys.executable)} "
        f'{shlex.quote(str(source))} "$@"\n',
        encoding="utf-8",
    )
    executable.chmod(executable.stat().st_mode | stat.S_IXUSR)


def write_fake_verus(tmp_path: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    """Create a verifier that records argv and can fail on a selected proof.

    Returns
    -------
    tuple[pathlib.Path, pathlib.Path]
        The executable verifier and the log it appends invocations to.
    """
    executable = tmp_path / "fake-verus"
    invocation_log = tmp_path / "verus-invocations.jsonl"
    write_launcher(
        executable,
        """\
        import json
        import os
        import pathlib
        import sys

        if sys.argv[1:] == ["--version"]:
            print("Verus\\nToolchain: nightly-2025-01-01")
            raise SystemExit(0)
        log = pathlib.Path(os.environ["VERUS_INVOCATION_LOG"])
        with log.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(sys.argv[1:]) + "\\n")
        if os.environ.get("VERUS_FAIL_PROOF") in sys.argv[1:]:
            raise SystemExit(int(os.environ.get("VERUS_FAIL_STATUS", "42")))
        """,
    )
    return executable, invocation_log


def make_proofs(tmp_path: pathlib.Path, count: int = 3) -> list[pathlib.Path]:
    """Create ``count`` existing files for ordered proof-run assertions.

    Returns
    -------
    list[pathlib.Path]
        The staged proof files, in creation order.
    """
    names = ["one", "two", "three", "four", "five"]
    proofs = [tmp_path / f"proof {names[index]}.rs" for index in range(count)]
    for proof in proofs:
        proof.write_text("// Fake verifier input.\n", encoding="utf-8")
    return proofs


def write_prover_tools_shim(tmp_path: pathlib.Path) -> pathlib.Path:
    """Create a single-word stand-in for the multi-word PROVER_TOOLS command.

    ``make`` substitutes ``PROVER_TOOLS`` into a recipe unquoted, so passing the
    real command as a make override would re-split and re-expand it. A shim
    that execs the command it reads from the environment keeps the override a
    single path while still driving the pinned CLI.

    Returns
    -------
    pathlib.Path
        The executable shim to pass as ``PROVER_TOOLS``.
    """
    shim = tmp_path / "prover-tools-shim"
    write_launcher(
        shim,
        """\
        import os
        import shlex
        import subprocess
        import sys

        argv = shlex.split(os.environ["PROVER_TOOLS_REAL"]) + sys.argv[1:]
        raise SystemExit(
            subprocess.run(argv, check=False, shell=False).returncode
        )
        """,
    )
    return shim


def run_verus(
    tmp_path: pathlib.Path,
    proofs: list[pathlib.Path],
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the pinned CLI with tool setup disabled and a fake Verus binary.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished CLI process, with output captured.
    """
    command = shlex.split(os.environ["PROVER_TOOLS"])
    executable, invocation_log = write_fake_verus(tmp_path)
    # Several examples may share one directory, so each run starts from an
    # empty log rather than inheriting the previous run's invocations.
    invocation_log.unlink(missing_ok=True)
    command.extend([
        "verus",
        "run",
        "--no-install-missing",
        "--no-ensure-toolchain",
        "--verus-bin",
        str(executable),
    ])
    for proof in proofs:
        command.extend(["--proof-file", str(proof)])

    child_environment = os.environ.copy()
    child_environment["VERUS_INVOCATION_LOG"] = str(invocation_log)
    if environment is not None:
        child_environment.update(environment)
    return run_command(command, cwd=REPOSITORY_ROOT, env=child_environment)


def invocations(invocation_log: pathlib.Path) -> list[list[str]]:
    """Read the fake verifier's one-JSON-array-per-process log.

    Returns
    -------
    list[list[str]]
        One argument list per verifier process that ran, in order.
    """
    if not invocation_log.exists():
        return []
    return [json.loads(line) for line in invocation_log.read_text().splitlines()]


def proof_for_invocation(
    invocation: list[str], proofs: list[pathlib.Path]
) -> pathlib.Path:
    """Return the one proof path passed to an invocation.

    Returns
    -------
    pathlib.Path
        The single proof file named in ``invocation``.
    """
    matches = [proof for proof in proofs if str(proof) in invocation]
    assert len(matches) == 1, f"expected one proof in argv, got {invocation!r}"
    return matches[0]


def run_make_verus(
    tmp_path: pathlib.Path,
    proofs: list[pathlib.Path],
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the repository's ``verus`` target against the fake verifier.

    The target, not just the CLI, is under test here: it is what CI and
    maintainers invoke, and its variable plumbing is only exercised by
    expanding it for real.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished ``make`` process, with output captured.
    """
    make = shutil.which("make")
    assert make is not None, "the target contract test requires make"
    executable, invocation_log = write_fake_verus(tmp_path)
    invocation_log.unlink(missing_ok=True)
    flags = shlex.join([
        "--no-install-missing",
        "--no-ensure-toolchain",
        "--verus-bin",
        str(executable),
        *(item for proof in proofs for item in ("--proof-file", str(proof))),
    ])
    child_environment = os.environ.copy()
    child_environment["PROVER_TOOLS_REAL"] = os.environ["PROVER_TOOLS"]
    child_environment["VERUS_INVOCATION_LOG"] = str(invocation_log)
    if environment is not None:
        child_environment.update(environment)
    return run_command(
        [
            make,
            "--no-print-directory",
            "-f",
            "Makefile",
            "verus",
            f"PROVER_TOOLS={write_prover_tools_shim(tmp_path)}",
            f"VERUS_FLAGS={flags}",
        ],
        cwd=REPOSITORY_ROOT,
        env=child_environment,
    )
