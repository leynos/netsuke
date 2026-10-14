"""Hold `kani-smoke` to the change-scoped shape ADR-039 records.

`kani-smoke` is a required check on the ruleset, so a pull request cannot
merge until it reports. Skipping the job itself, with a trigger `paths`
filter or a job-level `if:`, leaves a required check that never reports and a
pull request that can never merge. The job therefore always runs, its first
real step decides whether the proofs can differ from `main`'s, and every step
after the decision is conditioned on it. These contracts pin that shape: the
decision's position and command, the condition on every later step, the
absence of a job condition and trigger filters, and the nightly schedule that
runs the proofs in full.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import (
    job_steps,
    load_workflow,
    require_list,
    require_mapping,
    workflow_job,
)

JOB = "kani-smoke"
DECISION_STEP = "Decide Kani proof scope"
DECISION_COMMAND = "uv run --script scripts/kani_proof_scope.py"
#: The output the script writes; `kani_proof_scope_decision_test.py` runs the
#: script and reads this exact key back from `GITHUB_OUTPUT`.
PROOF_CONDITION = "steps.scope.outputs.run-proofs == 'true'"
#: The jobs other than `kani-smoke`, which the nightly schedule must not run.
SCHEDULE_EXCLUDED_JOBS = ("build-test", "windows")
NOT_ON_SCHEDULE = "github.event_name != 'schedule'"


@pytest.fixture(scope="module")
def workflow() -> dict[str, object]:
    """Return the CI workflow."""
    return load_workflow()


@pytest.fixture(scope="module")
def steps(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the `kani-smoke` steps."""
    return job_steps(workflow, JOB)


def _decision_index(steps: list[dict[str, object]]) -> int:
    """Return the position of the one decision step."""
    positions = [
        index for index, step in enumerate(steps) if step.get("name") == DECISION_STEP
    ]
    assert len(positions) == 1, f"`{JOB}` must have exactly one `{DECISION_STEP}` step"
    return positions[0]


def test_job_always_runs_and_reports(workflow: dict[str, object]) -> None:
    """Refuse a job condition, which would stop a required check reporting."""
    assert "if" not in workflow_job(workflow, JOB), (
        f"`{JOB}` is a required check; a job-level `if:` can leave it unreported"
    )


def test_triggers_carry_no_path_filters(workflow: dict[str, object]) -> None:
    """Refuse trigger path filters, which skip the whole workflow's checks."""
    triggers = require_mapping(workflow.get("on"), "ci.yml triggers")
    filtered = {
        event: key
        for event, settings in triggers.items()
        if isinstance(settings, dict)
        for key in ("paths", "paths-ignore")
        if key in settings
    }
    assert not filtered, f"ci.yml triggers must not filter paths: {filtered}"


def test_nightly_schedule_runs_only_the_proofs(workflow: dict[str, object]) -> None:
    """Require one nightly cron and every other job excluded from it."""
    triggers = require_mapping(workflow.get("on"), "ci.yml triggers")
    schedule = require_list(triggers.get("schedule"), "ci.yml schedule")
    crons = [require_mapping(entry, "schedule entry").get("cron") for entry in schedule]
    assert crons == ["41 4 * * *"], f"expected the one nightly cron, found {crons}"
    for job in SCHEDULE_EXCLUDED_JOBS:
        assert workflow_job(workflow, job).get("if") == NOT_ON_SCHEDULE, (
            f"`{job}` must skip the nightly schedule, which exists for `{JOB}` alone"
        )


def test_decision_reads_the_merge_commit(steps: list[dict[str, object]]) -> None:
    """Require the checkout to fetch the merge commit's first parent."""
    checkout = steps[0]
    assert str(checkout.get("uses", "")).startswith("actions/checkout@"), (
        f"`{JOB}` must start with the checkout"
    )
    assert (
        require_mapping(checkout.get("with"), "checkout inputs").get("fetch-depth") == 2
    ), "the scope decision diffs HEAD against HEAD^1, which needs `fetch-depth: 2`"


def test_decision_step_is_unconditional_and_runs_the_script(
    steps: list[dict[str, object]],
) -> None:
    """Require the decision to run on every trigger, as its step's sole command."""
    index = _decision_index(steps)
    decision = steps[index]
    assert decision.get("id") == "scope", "later steps read `steps.scope.outputs`"
    assert "if" not in decision, "the decision must run on every trigger"
    assert decision.get("run") == DECISION_COMMAND, (
        f"the decision must run `{DECISION_COMMAND}` as its sole command"
    )
    env = require_mapping(decision.get("env"), "decision env")
    assert env == {"INPUT_EVENT_NAME": "${{ github.event_name }}"}, (
        f"the decision must read the triggering event and nothing else: {env}"
    )
    before = [str(step.get("uses", "")).split("@", 1)[0] for step in steps[:index]]
    assert before == ["actions/checkout", "astral-sh/setup-uv"], (
        "only the checkout and uv may precede the decision, so a skipped run "
        f"pays for nothing else; found {before}"
    )
    assert all("if" not in step for step in steps[:index]), (
        "the steps the decision needs must run unconditionally"
    )


def test_every_later_step_is_conditioned_on_the_decision(
    steps: list[dict[str, object]],
) -> None:
    """Require the exact decision condition on every step after it.

    A later step without it runs its work on a skipped pull request; a step
    with any other condition may skip on a push to `main`, where the proofs
    must always run. The presence half requires the harness step itself, so
    that deleting it cannot satisfy the condition check vacuously.
    """
    later = steps[_decision_index(steps) + 1 :]
    assert any(step.get("run") == "make kani-ir" for step in later), (
        f"`{JOB}` must run `make kani-ir` after the decision"
    )
    unconditioned = [
        step.get("name") for step in later if step.get("if") != PROOF_CONDITION
    ]
    assert not unconditioned, (
        f"these `{JOB}` steps do not carry `if: {PROOF_CONDITION}`: {unconditioned}"
    )
