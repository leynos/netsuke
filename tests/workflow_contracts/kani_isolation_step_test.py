"""Execute the `kani-smoke` isolation step against stubbed host commands.

The step lifts Ubuntu's AppArmor unprivileged-userns restriction for its own
duration and restores it afterwards. Three things can go wrong, and each has a
different correct outcome: the restriction cannot be lifted (the boundary
cannot be exercised, so the step must fail before the suite runs), the suite
fails and the restriction is restored (the suite's status must survive), or the
suite passes and the restore fails (leaving the host modified must fail the
step). A restore failure alongside a suite failure keeps the suite's status,
which is the more specific diagnosis.

Reading the YAML cannot show any of that, so the checked-in `run` body is
executed with `make` and `sudo` replaced by stubs, and the restriction path
pointed at a temporary file.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import os

# ruff: ignore[suspicious-subprocess-import] - the checked-in workflow shell is the boundary under test.
import subprocess
import typing as typ

import pytest
from workflow_loading import job_steps, load_workflow, named_step, require_mapping

if typ.TYPE_CHECKING:
    from pathlib import Path

STEP_NAME = "Verify the kani-check isolation boundary"
JOB_NAME = "kani-smoke"
RESTRICTION_VARIABLE = "KANI_APPARMOR_RESTRICTION"

#: Records that `make` ran, copies the `sudo` calls made so far to the marker so
#: the test can prove the lift preceded the suite, and exits with
#: ``FAKE_MAKE_STATUS``.
FAKE_MAKE = """#!/bin/sh
set -eu
if [ -f "$FAKE_SUDO_LOG" ]; then cat "$FAKE_SUDO_LOG"; fi > "$FAKE_MAKE_MARKER"
exit "${FAKE_MAKE_STATUS:-0}"
"""

#: Refuses or accepts each `sysctl --write` through ``FAKE_SUDO_LIFT`` and
#: ``FAKE_SUDO_RESTORE``, and records the arguments it was given.
FAKE_SUDO = """#!/bin/sh
set -eu
printf '%s\\n' "$*" >> "$FAKE_SUDO_LOG"
case "$*" in
  *apparmor_restrict_unprivileged_userns=0) exit "${FAKE_SUDO_LIFT:-0}" ;;
  *apparmor_restrict_unprivileged_userns=1) exit "${FAKE_SUDO_RESTORE:-0}" ;;
esac
exit 0
"""


def _step_body() -> tuple[str, str]:
    """Return the isolation step's shell body and its restriction node path."""
    steps = job_steps(load_workflow(), JOB_NAME)
    step = named_step(steps, STEP_NAME)
    body = str(step.get("run", ""))
    assert body, f"{STEP_NAME} must declare a shell body"
    environment = require_mapping(step.get("env", {}), f"{STEP_NAME} env")
    restriction = environment.get(RESTRICTION_VARIABLE)
    assert isinstance(restriction, str), (
        f"{STEP_NAME} must set {RESTRICTION_VARIABLE} so the body has one source "
        f"for the sysctl node, got {restriction!r}"
    )
    assert restriction, f"{RESTRICTION_VARIABLE} must name a node, got {restriction!r}"
    return body, restriction


@dataclasses.dataclass(frozen=True, slots=True)
class ExitStatuses:
    """What each stubbed command reports, grouped so a case names only its own.

    The three failures are independent — a refused lift, a failing suite, and a
    refused restore happen at different points in the step — so each case sets
    the one it is about and leaves the rest at zero.
    """

    suite: int = 0
    lift: int = 0
    restore: int = 0


#: Every stub succeeding, which is the shape most cases start from.
ALL_SUCCEED = ExitStatuses()


#: The sysctl node's file name inside the sandbox.
RESTRICTION_FILE = "apparmor_restrict_unprivileged_userns"


def _read(path: Path) -> str:
    """Return the file's text, or an empty string when it was never written."""
    return path.read_text(encoding="utf-8") if path.exists() else ""


def _write_stubs(sandbox: Path) -> Path:
    """Install the ``make`` and ``sudo`` stubs and return the directory holding them."""
    fake_bin = sandbox / "fake-bin"
    fake_bin.mkdir()
    for name, script in (("make", FAKE_MAKE), ("sudo", FAKE_SUDO)):
        stub = fake_bin / name
        stub.write_text(script, encoding="utf-8")
        stub.chmod(0o755)
    return fake_bin


def _step_environment(
    sandbox: Path, fake_bin: Path, statuses: ExitStatuses
) -> dict[str, str]:
    """Return the environment the step body runs with, its stubs first on ``PATH``."""
    return {
        **os.environ,
        "PATH": os.pathsep.join([str(fake_bin), os.environ.get("PATH", "")]),
        RESTRICTION_VARIABLE: str(sandbox / RESTRICTION_FILE),
        "FAKE_MAKE_MARKER": str(sandbox / "make-ran"),
        "FAKE_MAKE_STATUS": str(statuses.suite),
        "FAKE_SUDO_LIFT": str(statuses.lift),
        "FAKE_SUDO_RESTORE": str(statuses.restore),
        "FAKE_SUDO_LOG": str(sandbox / "sudo.log"),
    }


def _run_step(
    sandbox: Path,
    *,
    restriction_contents: str | None,
    statuses: ExitStatuses = ALL_SUCCEED,
) -> tuple[subprocess.CompletedProcess[str], bool, str, str]:
    """Run the step body in ``sandbox`` and report what it did.

    Returns
    -------
    tuple[subprocess.CompletedProcess[str], bool, str, str]
        The completed process, whether the suite ran, the recorded `sudo`
        invocations, and the subset of them made before the suite started.
    """
    body, _ = _step_body()
    fake_bin = _write_stubs(sandbox)
    restriction = sandbox / RESTRICTION_FILE
    if restriction_contents is not None:
        restriction.write_text(f"{restriction_contents}\n", encoding="utf-8")

    # ruff: ignore[subprocess-without-shell-equals-true] - fixed checked-in workflow body.
    completed = subprocess.run(
        ["/bin/bash", "-e", "-o", "pipefail", "-c", body],
        check=False,
        capture_output=True,
        env=_step_environment(sandbox, fake_bin, statuses),
        shell=False,
        text=True,
    )
    marker = sandbox / "make-ran"
    suite_ran = marker.exists()
    return completed, suite_ran, _read(sandbox / "sudo.log"), _read(marker)


def _assert_reports_restore_failure(
    completed: subprocess.CompletedProcess[str],
) -> None:
    """Require the step to annotate a failed restore rather than swallow it."""
    combined = completed.stdout + completed.stderr
    assert "::error::" in combined, (
        f"a failed restore must raise a CI error annotation, got {combined!r}"
    )
    assert "cannot restore the AppArmor" in combined, (
        f"the annotation must name the restore that failed, got {combined!r}"
    )


@pytest.mark.parametrize(
    "restriction_contents", [None, "0"], ids=["node-absent", "node-unrestricted"]
)
def test_step_skips_lift_and_restore_without_a_restriction(
    tmp_path: Path, restriction_contents: str | None
) -> None:
    """Skip the lift and the restore on a host that does not restrict userns."""
    completed, ran, sudo_calls, before_the_suite = _run_step(
        tmp_path, restriction_contents=restriction_contents
    )

    assert completed.returncode == 0, completed.stderr
    assert ran, "the suite must run whether or not the restriction exists"
    assert not sudo_calls, f"no sysctl call is needed, got {sudo_calls!r}"
    assert not before_the_suite, (
        f"the suite must start without any sysctl call, got {before_the_suite!r}"
    )


def test_step_restores_the_restriction_it_lifted(tmp_path: Path) -> None:
    """A passing suite on a restricted host restores the sysctl and succeeds."""
    completed, ran, sudo_calls, before_the_suite = _run_step(
        tmp_path, restriction_contents="1"
    )

    assert completed.returncode == 0, completed.stderr
    assert ran, "the suite must run once the restriction is lifted"
    assert "unprivileged_userns=0" in before_the_suite, (
        "the restriction must be lifted before the suite runs, "
        f"got {before_the_suite!r}"
    )
    assert "unprivileged_userns=1" not in before_the_suite, (
        f"the suite must not start after the restriction is restored, "
        f"got {before_the_suite!r}"
    )
    assert "unprivileged_userns=1" in sudo_calls, (
        f"the lifted restriction must be restored, got {sudo_calls!r}"
    )


def test_step_preserves_a_suite_failure(tmp_path: Path) -> None:
    """A failing suite keeps its own status after a successful restore."""
    completed, ran, _, _ = _run_step(
        tmp_path, restriction_contents="1", statuses=ExitStatuses(suite=7)
    )

    assert ran, "the suite must run before it can fail"
    assert completed.returncode == 7, (
        f"the suite's status must survive the restore, got {completed.returncode}"
    )


def test_step_fails_when_the_restore_fails(tmp_path: Path) -> None:
    """A passing suite cannot hide a restore that left the host modified."""
    completed, _, sudo_calls, _ = _run_step(
        tmp_path, restriction_contents="1", statuses=ExitStatuses(restore=1)
    )

    assert "unprivileged_userns=1" in sudo_calls, (
        f"the restore must still be attempted, got {sudo_calls!r}"
    )
    assert completed.returncode != 0, (
        "a failed restore must fail the step even when the suite passed"
    )
    _assert_reports_restore_failure(completed)


def test_suite_failure_outranks_a_restore_failure(tmp_path: Path) -> None:
    """A suite failure keeps its status while the restore failure is reported."""
    completed, _, _, _ = _run_step(
        tmp_path,
        restriction_contents="1",
        statuses=ExitStatuses(suite=5, restore=1),
    )

    assert completed.returncode == 5, (
        f"the suite's status is the more specific diagnosis, got {completed.returncode}"
    )
    _assert_reports_restore_failure(completed)


def test_step_fails_before_the_suite_when_the_lift_is_refused(
    tmp_path: Path,
) -> None:
    """An unliftable restriction fails loudly without reporting a sandbox fault."""
    completed, ran, sudo_calls, _ = _run_step(
        tmp_path, restriction_contents="1", statuses=ExitStatuses(lift=1)
    )

    assert not ran, "the suite must not run when the boundary cannot be exercised"
    assert completed.returncode != 0, completed.stderr
    combined = completed.stdout + completed.stderr
    assert "::error::" in combined, (
        f"the refusal must be an actionable CI error, got {combined!r}"
    )
    assert "cannot lift the AppArmor" in combined, (
        f"the error must name the operation that was refused, got {combined!r}"
    )
    assert "unprivileged_userns=1" not in sudo_calls, (
        f"nothing was lifted, so nothing may be restored, got {sudo_calls!r}"
    )
