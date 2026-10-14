r"""Pin what the release-admission runners return for a real child.

Every other case in the suite reaches :mod:`bounding` through the gate, where a
cmd-mox shim supplies the status and both streams. Those cases therefore cannot
distinguish a result built from a real child from a result a double fabricated,
and they cannot see an *inherited* stream at all: the shim's standard error is
written inside the child, and only the process that owns the file descriptor can
observe where it went.

These cases run real children instead. The distinction they pin is the one the
gate's stream wiring rests on:

- A runner that *captures* standard output returns that text, and reports
  standard error as ``None`` -- because the child's standard error goes to the
  gate's own, it is not in the pipe.
- A runner that *inherits* both streams returns ``None`` for each, because
  there was no stream of ours to read, while the text still reaches the gate's
  file descriptor.

The status contracts are in :mod:`test_release_admission_status_translation`,
which needs no child; this module needs one, which is why the two are separate.

Scope: every case here calls :mod:`bounding` *directly* through
``run_bounded_sync`` and ``run_unbounded_sync``. Nothing in this module covers
the gate's dependency provisioning or its shebang execution -- both are
properties of invoking ``release_admission.py`` as ``uv run --script``, which
this module never does. The entry point's runtime contract is asserted
statically in
:mod:`test_release_admission_metrics`, and the whole-gate boundary is
:mod:`release_admission_test_harness`'s.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest scripts/tests/test_release_admission_runner_contract.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import os
import stat
import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
import cuprum
from _release_admission import bounding

#: The status a real child exits with, chosen so it cannot be confused with a
#: launch refusal's ``126`` or ``127``.
CHILD_STATUS = 3


def _program(tmp_path: Path, name: str, body: str, *, executable: bool = True) -> str:
    """Write one child script and return its path.

    Returns
    -------
    str
        The path the command under test is built from.
    """
    path = tmp_path / name
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755 if executable else stat.S_IRUSR | stat.S_IWUSR)
    return str(path)


def _command(path: str) -> cuprum.SafeCmd:
    """Build the allow-listed command a runner is exercised with.

    Returns
    -------
    cuprum.SafeCmd
        The command, built through a catalogue that admits exactly this
        program. The catalogue is explicit because the production gate builds
        one from its configured adapter names in the same way; cuprum's default
        catalogue admits only the estate's own commands, and a scratch child is
        not one of them.
    """
    program = cuprum.Program(path)
    catalogue = cuprum.ProgramCatalogue(
        projects=[
            cuprum.ProjectSettings(
                name="runner-contract-probe",
                programs=(program,),
                documentation_locations=(),
                noise_rules=(),
            )
        ]
    )
    return cuprum.sh.make(program, catalogue=catalogue)()


def test_bound_and_unbound_runners_report_a_real_childs_status(
    tmp_path: Path,
) -> None:
    """Verify both runners build a result from a status the kernel reported.

    Notes
    -----
    The two runners take different paths through the module -- one waits on the
    child's exit, the other drains its captured output -- so a status asserted
    through only one of them would leave the other free to report anything. The
    child exits with a status that is neither success nor a launch refusal, so
    an implementation that defaulted either value fails here.
    """
    command = _command(
        _program(tmp_path, "exiting-three", f"#!/bin/sh\nexit {CHILD_STATUS}\n")
    )

    bounded = bounding.run_bounded_sync(command, timeout_seconds=10)
    unbounded = bounding.run_unbounded_sync(command)

    assert bounded is not None, "a child that exits within its bound returns a result"
    assert bounded.exit_code == CHILD_STATUS, (
        "the bounded runner must report the child's own status"
    )
    assert unbounded.exit_code == CHILD_STATUS, (
        "the unbounded runner must report the child's own status"
    )


def test_capture_is_private_and_inheritance_is_reported_as_absent(
    tmp_path: Path,
) -> None:
    """Verify the two runners disagree about the stream they capture, by design.

    Notes
    -----
    A bound that captures standard output builds a result carrying that text; a
    call that inherits it builds a result with no text at all, because there was
    no stream of ours to read. The gate depends on the difference in both
    directions: the parsed API answer must stay private, and an operator's
    adapter diagnostic must reach the runner log. Both results are therefore
    inspected side by side, from one child, so neither property can be satisfied
    by making the two runners identical.

    Standard error is inherited in *both* modes, so it is reported as absent in
    both. That is the shell's own arrangement -- a command substitution captured
    standard output alone -- and a result that carried a string there would
    mean the gate had begun capturing a stream the shell left alone.
    """
    command = _command(
        _program(
            tmp_path,
            "reporting",
            "#!/bin/sh\nprintf 'readable\\n'\nprintf 'diagnostic\\n' >&2\n",
        )
    )

    captured = bounding.run_bounded_sync(command, timeout_seconds=10)
    inherited = bounding.run_bounded_sync(
        command, timeout_seconds=10, inherit_streams=True
    )

    assert captured is not None, "a capturing run returns a result"
    assert captured.stdout == "readable\n", (
        "a bound that captures standard output must return what the child wrote"
    )
    assert captured.stderr is None, (
        "an inherited standard error must be reported as absent in both modes"
    )
    assert inherited is not None, "an inheriting run returns a result"
    assert inherited.stdout is None, (
        "an inherited stream must be reported as absent, not as empty text"
    )
    assert inherited.stderr is None, (
        "an inherited standard error must be reported as absent too"
    )


def test_inherited_streams_reach_the_processs_own_descriptors(tmp_path: Path) -> None:
    """Verify both inherited streams are written where the shell left them.

    Notes
    -----
    A runner that reported ``None`` for an inherited stream while *discarding*
    the text would satisfy the case above and still lose an operator's
    diagnostic -- and the ``git`` fetch's own report with it, since the shell
    redirected neither of that command's streams.

    The child inherits the *descriptors* rather than the test process's stream
    objects, so the redirection is done at the descriptor level: the process's
    own standard output and standard error are each pointed at a scratch file
    for the duration of the run. Those files stand in for the runner log.
    """
    command = _command(
        _program(
            tmp_path,
            "reporting",
            "#!/bin/sh\nprintf 'report\\n'\nprintf 'diagnostic\\n' >&2\n",
        )
    )

    captured = _inherited_output(tmp_path, command)

    assert captured.stdout == "report\n", (
        "an unredirected standard output must reach the gate's own"
    )
    assert captured.stderr == "diagnostic\n", (
        "an unredirected standard error must reach the gate's own"
    )


class _InheritedOutput(typ.NamedTuple):
    """Hold what one child wrote to the two descriptors it inherited."""

    stdout: str
    stderr: str


def _inherited_output(tmp_path: Path, command: cuprum.SafeCmd) -> _InheritedOutput:
    """Run *command*, returning what it wrote to the process's own descriptors.

    Returns
    -------
    _InheritedOutput
        The two streams the runner did not redirect.

    Notes
    -----
    Each original descriptor is restored in a ``finally``, so a failing
    assertion cannot leave the suite writing into a scratch file. The files are
    flushed by closing them rather than by ``fsync``: the child's own writes
    reached them through the duplicated descriptor and are already visible to a
    subsequent reader once the child has been reaped.
    """
    paths = {
        "stdout": tmp_path / "inherited-stdout",
        "stderr": tmp_path / "inherited-stderr",
    }
    originals = {"stdout": os.dup(1), "stderr": os.dup(2)}
    try:
        with paths["stdout"].open("wb") as out, paths["stderr"].open("wb") as err:
            os.dup2(out.fileno(), 1)
            os.dup2(err.fileno(), 2)
            try:
                bounding.run_bounded_sync(
                    command, timeout_seconds=10, inherit_streams=True
                )
            finally:
                os.dup2(originals["stdout"], 1)
                os.dup2(originals["stderr"], 2)
    finally:
        for descriptor in originals.values():
            os.close(descriptor)
    return _InheritedOutput(
        stdout=paths["stdout"].read_text(encoding="utf-8"),
        stderr=paths["stderr"].read_text(encoding="utf-8"),
    )


def test_unexecutable_program_is_reported_by_the_runner(tmp_path: Path) -> None:
    """Verify a refused launch reaches the caller as an error, not a status.

    Notes
    -----
    The runner does not translate a launch refusal; :mod:`commands` does, by
    catching the error and asking :func:`statuses.launch_failure_status`. Pinning
    the division of labour is the point: the runner raises, so a caller that
    forgot to catch is a visible fault rather than a silently adopted status.
    """
    command = _command(
        _program(tmp_path, "no-execute-bit", "#!/bin/sh\n", executable=False)
    )

    with pytest.raises(PermissionError):
        bounding.run_bounded_sync(command, timeout_seconds=10)
