"""Hold the verification job's telemetry to the release observability contract.

Verification is where a release is first compared with the record the
publisher kept, so its phases are what an operator reads after a refusal. The
job must therefore report the listing, the downloads, and the verifier
separately, with bounded outcomes and timings, and retain the record past the
runner. The action is the only place that sees those boundaries, so the job's
summary has to read them from the action's outputs rather than guess.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_operation_catalog import OPERATION_PHASES
from release_operation_observability_support import (
    VERIFICATION_PHASES,
    assert_retained_telemetry_policy,
    assert_summary_environment,
    assert_summary_phase_timings,
    assert_verification_phases_read_action_outputs,
)
from release_verify_inventory_support import (
    DOWNLOAD_STEP,
    LIST_ASSETS_STEP,
    LIST_RELEASES_STEP,
    VERIFY_STEP,
    load_action,
    load_action_steps,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
)

#: The job whose telemetry this module holds to account.
VERIFY_JOB = "verify-release-inventory"


@pytest.fixture(scope="module")
def release_workflow() -> dict[str, object]:
    """Load the release workflow once for the verification wiring checks."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def test_verification_job_retains_phase_observability(
    release_workflow: dict[str, object],
) -> None:
    """Retain the verifier's phases in an always-run summary and JSONL artifact.

    Verification runs only when the run is publishing, so the mode is fixed
    rather than read from an output that could disagree with the guard.
    """
    steps = job_steps(release_workflow, VERIFY_JOB)
    env = assert_summary_environment(
        steps, "verify_release_inventory", "false", staged=False
    )
    assert_summary_phase_timings(env, VERIFICATION_PHASES)
    assert_verification_phases_read_action_outputs(env)
    assert_retained_telemetry_policy(steps)


def test_each_timed_step_records_its_own_span() -> None:
    """Require every reported phase to be timed where the action runs it.

    The listing phase spans two steps, so it starts in the release listing and
    ends in the asset listing; the other two phases bracket their own step. A
    phase whose markers are missing would be reported as an unavailable
    duration, which is exactly the fact an operator needs after a refusal.
    """
    steps = load_action_steps(load_action())
    for name, writes_start in (
        (LIST_RELEASES_STEP, True),
        (LIST_ASSETS_STEP, False),
        (DOWNLOAD_STEP, True),
        (VERIFY_STEP, True),
    ):
        run = str(named_step(steps, name).get("run"))
        assert run.splitlines()[0] == "set -euo pipefail", (
            f"{name} must enable strict shell handling before timing"
        )
        assert ("started_ns=" in run) is writes_start, (
            f"{name} must {'record' if writes_start else 'not record'} a start time"
        )
        assert "ended_ns=" in run, f"{name} must record an end time"


def test_the_verification_operation_reports_only_its_own_phases() -> None:
    """Keep the catalogue's verification phases to the three the job runs.

    A phase the job does not run would be reported as skipped on every run,
    which reads as a failure that never happened.
    """
    assert OPERATION_PHASES["verify_release_inventory"] == VERIFICATION_PHASES
