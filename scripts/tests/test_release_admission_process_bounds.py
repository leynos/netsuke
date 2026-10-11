r"""Verify the release-admission bound reaches every process it started.

The gate's own bound is the one piece of its behaviour that a cmd-mox shim
cannot stand in for: a shim is a script the test does not own, and none of the
contracts here can express a child that refuses to leave. Both cases therefore
run the real helper against a real child that traps ``TERM``.

The stronger case is the regression test for the defect the shell's GNU
``timeout`` hid. cuprum spawns without ``start_new_session`` and terminates
only the direct child, so a descendant survives cancellation; worse, a
survivor that inherited the captured stdout keeps the pipe open, the drain
never reaches end-of-file, and a two-second bound overran by 120 seconds when
measured. Both halves of the restored guarantee are asserted together, because
either alone would pass a broken bound.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest \
        scripts/tests/test_release_admission_process_bounds.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import os
import pathlib
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
import cuprum
import cuprum.sh
from _release_admission import bounding, commands

#: How long the child reports nothing for before the bound must escalate.
ESCALATION_SECONDS = 1
#: How much longer than ``ESCALATION_SECONDS + CANCEL_GRACE_SECONDS`` the whole
#: call may take before the bound has demonstrably failed. The allowance is
#: generous enough for a loaded machine to schedule the escalation, and far
#: below the overrun a surviving descendant that holds the captured pipes
#: produces: the port measured that at 120 seconds against a 2 second bound.
OVERRUN_ALLOWANCE_SECONDS = 10.0
#: A child that traps ``TERM``, records its own process ID, and then waits.
#:
#: The descendant is the load-bearing part. It inherits the child's streams --
#: including the pipe the bound captures -- and traps ``TERM`` as well, so it
#: keeps that pipe open after the direct child is signalled. A bound that
#: signals only the direct child therefore never reaches end-of-file on the
#: drain and overruns for as long as the descendant lives, which is the whole
#: defect this pair of assertions exists to catch. The probe starts the
#: descendant named by its second argument, and the descendant records its own
#: pid there, so the test can check the two cleanups separately.
UNKILLABLE_CHILD = """#!/usr/bin/env bash
trap '' TERM
printf '%s\\n' "$$" >"$1"
"$2" "$1.desc" &
while :; do sleep 1; done
"""

#: The descendant the probe starts. It too ignores ``TERM``, so the bound's
#: ``SIGKILL`` escalation is the only thing that can end it.
UNKILLABLE_DESCENDANT = """#!/usr/bin/env bash
trap '' TERM
printf '%s\\n' "$$" >"$1"
while :; do sleep 1; done
"""


def _launch_bound(
    command: cuprum.SafeCmd, timeout_seconds: int
) -> cuprum.CommandResult | None:
    """Run *command* under the gate's own bound and return its result.

    Parameters
    ----------
    command
        The allow-listed child the gate would run.
    timeout_seconds
        The bound the gate configures for this operation.

    Returns
    -------
    cuprum.CommandResult | None
        The completed command, or ``None`` when the bound elapsed.
    """
    return commands.run_bounded_sync(command, timeout_seconds=timeout_seconds)


def _process_exists(pid: int) -> bool:
    """Return whether the pid still names a live process.

    Parameters
    ----------
    pid
        Process identifier the probe child recorded for itself.

    Returns
    -------
    bool
        Whether the process is still there, which is how the escalation's reap
        is observed.

    Notes
    -----
    Signal ``0`` is the portable liveness question: it performs the permission
    and existence checks without delivering a signal, and raises
    :class:`ProcessLookupError` exactly when no such process remains. It
    replaces an earlier ``ps`` probe, which answered the same question but
    depended on a binary at a fixed path and on how that binary spells its
    output. The child is reaped by the call under test, so the zone where it
    could vanish between this check and the previous assertion belongs to that
    call rather than here.

    A process the kernel has reaped but whose identifier another process has
    since reused would be reported as alive. The window is microscopic and the
    mistake is in the safe direction: it fails this test rather than passing a
    broken one.
    """
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


def _bounded_probe_command(
    child: pathlib.Path, pid_file: pathlib.Path
) -> cuprum.SafeCmd:
    """Build the allow-listed probe command the bound is exercised against.

    Parameters
    ----------
    child
        The probe script that traps ``TERM`` and starts a descendant.
    pid_file
        Where the child and its descendant record their process identifiers.

    Returns
    -------
    cuprum.SafeCmd
        The command: the pid file, then the descendant the probe must start.
    """
    program = cuprum.Program(str(child))
    catalogue = cuprum.ProgramCatalogue(
        projects=[
            cuprum.ProjectSettings(
                name="bounded-run-probe",
                programs=(program,),
                documentation_locations=(),
                noise_rules=(),
            )
        ]
    )
    descendant = child.with_name(f"{child.name}.desc")
    return cuprum.sh.make(program, catalogue=catalogue)(str(pid_file), str(descendant))


def _write_probe(child: pathlib.Path) -> None:
    """Write the unkillable probe and its descendant beside it, both executable.

    The descendant is written next to the probe under the ``.desc`` suffix the
    probe is given as its second argument, so the two travel as a set.
    """
    child.write_text(UNKILLABLE_CHILD, encoding="utf-8")
    child.chmod(0o755)
    descendant = child.with_name(f"{child.name}.desc")
    descendant.write_text(UNKILLABLE_DESCENDANT, encoding="utf-8")
    descendant.chmod(0o755)


def _recorded_pid(pid_file: pathlib.Path) -> int:
    """Return the process identifier a probe recorded in *pid_file*."""
    return int(pid_file.read_text(encoding="utf-8").strip())


def test_bounded_run_escalates_against_a_child_that_ignores_sigterm(
    tmp_path: pathlib.Path,
) -> None:
    """Verify the admission bound terminates a child that ignores ``SIGTERM``.

    Notes
    -----
    GNU ``timeout --kill-after=1s`` escalated to ``SIGKILL``, and the port must
    escalate identically, because a wedged adapter would otherwise hang the
    release job rather than classify as a timeout. A cmd-mox shim cannot stand
    in for the child: it is a fixed script the test does not own, so the
    escalation is exercised directly against the helper with a real child that
    traps ``TERM`` and reports its own process ID.

    Contract invariants: the bound reports ``None`` rather than an exit status,
    the gate's classification of that is ``124``, and the child is gone once the
    call returns -- reaped, not merely signalled.
    """
    child = tmp_path / "stubborn"
    _write_probe(child)
    pid_file = tmp_path / "pid"

    result = _launch_bound(_bounded_probe_command(child, pid_file), ESCALATION_SECONDS)

    assert result is None, "a bound that elapsed must report no result, not a status"
    assert commands.classify_result(result) == 124, (
        "an elapsed bound must classify as the timeout status"
    )
    assert not _process_exists(_recorded_pid(pid_file)), (
        "the escalate-to-kill path must leave no surviving child"
    )


def test_bounded_run_reaps_a_descendant_that_holds_the_captured_pipe(
    tmp_path: pathlib.Path,
) -> None:
    """Verify the bound reaches a descendant, within the bound, as GNU timeout did.

    Notes
    -----
    This is the regression test for the defect the shell's ``timeout`` hid.
    cuprum spawns without ``start_new_session`` and terminates only the direct
    child, so a descendant survives cancellation; worse, a survivor that
    inherited the captured stdout keeps the pipe open, the drain never reaches
    end-of-file, and a two-second bound overran by 120 seconds when measured.

    Both halves are asserted, because either alone would pass a broken bound.
    An elapsed bound with a live descendant is the old leak; a dead descendant
    that took four minutes to reach is the old overrun. The child and its
    descendant are checked separately: the direct child is reaped by
    ``process.wait()`` either way, so only the descendant distinguishes a
    group signal from a pid signal.

    A ``setsid()``-escaping descendant still escapes, which is a limitation the
    developers' guide records for the Kani scope wrapper as well; this probe
    stays in the group, as every ordinary adapter does.

    The bound is ``ESCALATION_SECONDS`` plus the helper's own cancel grace, and
    the elapsed check allows ``OVERRUN_ALLOWANCE_SECONDS`` on top for
    scheduling. The two are not in tension: measured, the call returns in
    ``bound + grace`` and the descendant is already gone.
    """
    child = tmp_path / "stubborn"
    _write_probe(child)
    pid_file = tmp_path / "pid"

    started = time.monotonic()
    result = _launch_bound(_bounded_probe_command(child, pid_file), ESCALATION_SECONDS)
    elapsed = time.monotonic() - started

    limit = (
        ESCALATION_SECONDS + bounding.CANCEL_GRACE_SECONDS + OVERRUN_ALLOWANCE_SECONDS
    )
    assert elapsed < limit, (
        f"a surviving descendant holding the captured pipe must not delay the "
        f"bound: {elapsed:.2f}s against a {limit:.2f}s limit"
    )
    assert result is None, "a bound that elapsed must report no result, not a status"
    assert commands.classify_result(result) == 124, (
        "an elapsed bound must classify as the timeout status"
    )
    descendant_pid = _recorded_pid(pid_file.with_name(f"{pid_file.name}.desc"))
    assert not _process_exists(descendant_pid), (
        "the group signal must reach a descendant the direct child started"
    )
