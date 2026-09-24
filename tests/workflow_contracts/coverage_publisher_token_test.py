"""The CodeScene token reaches the publisher's upload, and publishers queue.

The upload is `upload-codescene-coverage`, a composite action. A composite
action's nested steps inherit the calling step's environment, so a token in
the upload step's `env`, or the job's, or the workflow's, reaches every step
inside the action. `coverage-main.yml` therefore keeps the token out of every
`env`. A check step publishes only whether the token exists, the upload's
condition reads that output (held by `coverage_upload_guard_test`), and the
upload takes the token directly as an input.

The positive half matters as much as the prohibition: deleting the token
entirely satisfies "no `env` holds it" while the upload's guard goes false and
publishing silently stops. So the token is named exactly twice in the
publisher, in the check step's command and in the upload's input.

Publishers also serialize on one concurrency group per ref without cancelling
a run in progress, so two never race or abandon a baseline write.

Run via ``make test-workflow-contracts``.
"""

from ci_coverage_wiring_invariants import CREDENTIAL_ENVIRONMENT_KEY
from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)
from yaml_strings import iter_strings

JOB = "coverage-upload"
AVAILABILITY_STEP = "Check CodeScene token availability"
AVAILABILITY_STEP_ID = "codescene_token"
AVAILABILITY_COMMAND = (
    'echo "available=${{ secrets.CS_ACCESS_TOKEN != \'\' }}" >> "$GITHUB_OUTPUT"'
)
UPLOAD_STEP = "Upload coverage data to CodeScene"
UPLOAD_CREDENTIAL_INPUT = "${{ secrets.CS_ACCESS_TOKEN }}"
PUBLISHER_GROUP = "coverage-main-${{ github.ref }}"


def test_the_check_step_publishes_availability_and_nothing_else() -> None:
    """Run one command that writes a boolean, unconditionally, with no env.

    A condition on the check would leave its output unset whenever the
    condition was false, and an `env` on it would put the token back into an
    environment.
    """
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), JOB)
    check = named_step(steps, AVAILABILITY_STEP)
    assert check.get("id") == AVAILABILITY_STEP_ID, (
        f"the check must carry id {AVAILABILITY_STEP_ID!r}, got {check.get('id')!r}"
    )
    assert str(check.get("run", "")).strip() == AVAILABILITY_COMMAND, (
        f"the check must run exactly {AVAILABILITY_COMMAND!r}, got {check.get('run')!r}"
    )
    assert "if" not in check, "the check must run unconditionally"
    assert "env" not in check, "the check must declare no env"
    assert steps.index(check) < steps.index(named_step(steps, UPLOAD_STEP)), (
        "the check must run before the upload that reads its output"
    )


def test_the_upload_takes_the_token_directly() -> None:
    """Pass the secret as the action's input, not through an environment."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), JOB)
    inputs = require_mapping(named_step(steps, UPLOAD_STEP).get("with"), "upload")
    assert inputs.get("access-token") == UPLOAD_CREDENTIAL_INPUT, (
        f"the upload's access-token must be {UPLOAD_CREDENTIAL_INPUT!r}, got "
        f"{inputs.get('access-token')!r}"
    )


def test_no_environment_on_the_publisher_holds_the_token() -> None:
    """Keep the token out of the workflow, job and step environments."""
    workflow = load_workflow(COVERAGE_MAIN_WORKFLOW_PATH)
    jobs = require_mapping(workflow.get("jobs"), "publisher jobs")
    envs: list[object] = [workflow.get("env")]
    for name, job in jobs.items():
        envs.append(require_mapping(job, name).get("env"))
        envs.extend(step.get("env") for step in job_steps(workflow, name))
    holders = [
        env
        for env in envs
        if any(CREDENTIAL_ENVIRONMENT_KEY in text for text in iter_strings(env))
    ]
    assert holders == [], (
        f"no env on the publisher may name {CREDENTIAL_ENVIRONMENT_KEY}, found "
        f"{holders!r}; a composite action's nested steps inherit the calling "
        "step's env"
    )


def test_the_token_appears_exactly_where_it_is_used() -> None:
    """Name the token in the check's command and the upload's input, only."""
    mentions = sorted(
        text.strip()
        for text in iter_strings(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH))
        if CREDENTIAL_ENVIRONMENT_KEY in text
    )
    assert mentions == sorted([AVAILABILITY_COMMAND, UPLOAD_CREDENTIAL_INPUT]), (
        f"the publisher must name {CREDENTIAL_ENVIRONMENT_KEY} exactly in the "
        f"check command and the upload input, got {mentions!r}"
    )


def test_publishers_queue_on_one_group_per_ref() -> None:
    """Serialize publishers per ref, and never cancel one in progress."""
    concurrency = require_mapping(
        load_workflow(COVERAGE_MAIN_WORKFLOW_PATH).get("concurrency"),
        "publisher concurrency",
    )
    assert concurrency.get("group") == PUBLISHER_GROUP, (
        f"the publisher group must be {PUBLISHER_GROUP!r}, "
        f"got {concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        "the publisher must not cancel a run in progress, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )
