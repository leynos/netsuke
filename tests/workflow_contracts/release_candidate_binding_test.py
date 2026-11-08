"""Bind every release producer to the commit the declaration will name.

A suffix or a per-platform count says what an artefact *is*; it says nothing
about which commit produced it. The release therefore resolves one candidate
commit in the metadata job, hands it to every producer, and has each producer
verify the checkout it actually built. These tests pin that chain so a later
edit cannot leave a producer building whatever the triggering ref happened to
resolve to.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

CANDIDATE_STEP = "Bind the release candidate commit"
CHECKOUT_STEP = "Verify the checkout is the release candidate"
CANDIDATE_OUTPUT_EXPRESSION = "${{ steps.candidate_commit.outputs.value }}"
CANDIDATE_FORWARD = "${{ needs.metadata.outputs.candidate_commit }}"
PUBLISH_JOB = "publish-release"
UPLOAD_STEP = "Upload artefacts to release"
UPLOAD_ACTION = "./.github/actions/upload-declared-assets"
VERSION_FORWARD = "${{ needs.metadata.outputs.version }}"
PRODUCER_JOBS = ("build-linux", "build-windows", "build-macos")

#: What the HEAD comparison must contain, paired with why each fragment
#: matters. One parametrized row per fragment names the part that was lost,
#: where three inline probes in one body would report only the last.
HEAD_CHECK_FRAGMENTS = (
    ("git rev-parse HEAD", "the check must read the real HEAD"),
    ('"$CANDIDATE_COMMIT"', "the check must compare against the input"),
    ("exit 1", "a mismatched checkout must fail the producer"),
)

#: Matches the character class and the length check the step runs, so a
#: mutation to either half fails here rather than in a release run.
_FORBIDDEN_CHARACTER_CHECK = re.compile(r"\*\[!0-9a-f\]\*")
_LENGTH_CHECK = re.compile(r'"?\$\{#CANDIDATE_COMMIT\}"?\s+-ne\s+40')


def _checks_out_candidate(step: dict[str, object]) -> bool:
    """Report whether a checkout step pins ``ref`` to the candidate."""
    checkout_inputs = require_mapping(step.get("with"), "checkout inputs")
    return checkout_inputs.get("ref") == "${{ inputs.commit }}" or (
        checkout_inputs.get("ref") == CANDIDATE_FORWARD
    )


def test_metadata_job_publishes_the_candidate_commit() -> None:
    """The metadata job must resolve and expose the candidate commit."""
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    outputs = require_mapping(
        workflow_job(workflow, "metadata").get("outputs"), "metadata outputs"
    )
    assert outputs.get("candidate_commit") == CANDIDATE_OUTPUT_EXPRESSION, (
        "metadata must publish the candidate commit it validated"
    )


def test_metadata_candidate_step_validates_a_full_commit() -> None:
    """The candidate must be a full lowercase hexadecimal commit."""
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), "metadata")
    step = named_step(steps, CANDIDATE_STEP)
    assert "uses" not in step, "the candidate must be validated by local policy"
    env = require_mapping(step.get("env"), "the candidate step's environment")
    assert env.get("CANDIDATE_COMMIT") == "${{ github.sha }}", (
        "the candidate must be read from the run's own commit"
    )
    run = str(step.get("run", ""))
    assert _FORBIDDEN_CHARACTER_CHECK.search(run), (
        "the candidate must be rejected unless it is lowercase hexadecimal"
    )
    assert _LENGTH_CHECK.search(run), (
        "the candidate must be rejected unless it is a full 40-character commit"
    )


def test_called_workflow_requires_the_candidate() -> None:
    """The reusable workflow must take the candidate as a required input."""
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    triggers = require_mapping(workflow.get("on"), "the workflow triggers")
    call = require_mapping(triggers.get("workflow_call"), "workflow_call")
    inputs = require_mapping(call.get("inputs"), "workflow_call inputs")
    commit = require_mapping(inputs.get("commit"), "the commit input")
    assert commit.get("required") is True, (
        "a producer must not run without a named candidate"
    )
    assert commit.get("type") == "string", "the candidate is a commit string"


def test_producer_checkout_and_head_check_bind_the_candidate() -> None:
    """Each producer must check out the candidate and prove HEAD matches."""
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    steps = job_steps(workflow, "build")
    checkout = steps[0]
    assert "actions/checkout@" in str(checkout.get("uses", "")), (
        "the producer must start by checking out the candidate"
    )
    assert _checks_out_candidate(checkout), (
        "the producer checkout must pin ref to the requested commit"
    )
    checkout_inputs = require_mapping(checkout.get("with"), "checkout inputs")
    assert checkout_inputs.get("persist-credentials") is False, (
        "the producer checkout must not keep a push credential"
    )

    check = named_step(steps, CHECKOUT_STEP)
    assert steps.index(check) == steps.index(checkout) + 1, (
        "the HEAD comparison must immediately follow the checkout"
    )
    env = require_mapping(check.get("env"), "the HEAD check's environment")
    assert env.get("CANDIDATE_COMMIT") == "${{ inputs.commit }}", (
        "the HEAD check must compare against the requested candidate"
    )


@pytest.mark.parametrize(("fragment", "reason"), HEAD_CHECK_FRAGMENTS)
def test_the_head_check_compares_the_checkout(fragment: str, reason: str) -> None:
    """Every part of the HEAD comparison must survive an edit."""
    check = named_step(
        job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build"), CHECKOUT_STEP
    )
    run = str(check.get("run", ""))
    assert fragment in run, reason


@pytest.mark.parametrize("job_name", PRODUCER_JOBS)
def test_every_platform_caller_forwards_the_candidate(job_name: str) -> None:
    """No platform call site may omit the candidate."""
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    job = workflow_job(workflow, job_name)
    with_inputs = require_mapping(job.get("with"), f"{job_name} inputs")
    assert with_inputs.get("commit") == CANDIDATE_FORWARD, (
        f"{job_name} must pass the validated candidate to its producer"
    )


def test_crate_package_job_builds_the_candidate() -> None:
    """The crate archive must come from the same commit as the binaries."""
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    steps = job_steps(workflow, "package-crate")
    checkout = next(
        step for step in steps if "actions/checkout@" in str(step.get("uses", ""))
    )
    checkout_inputs = require_mapping(checkout.get("with"), "checkout inputs")
    assert checkout_inputs.get("ref") == CANDIDATE_FORWARD, (
        "the packaging checkout must pin the candidate"
    )


def test_the_workflow_call_does_not_fall_back_to_the_run_sha() -> None:
    """A producer must not discover its commit from the triggering ref."""
    text = PACKAGE_WORKFLOW_PATH.read_text(encoding="utf-8")
    assert "github.sha" not in text, (
        "the reusable workflow must take the candidate from its caller"
    )


def test_publication_action_receives_the_candidate_and_version() -> None:
    """Give the declaration step the candidate it binds records to.

    The action compares every producer record's ``commit`` against this input,
    so an action wired without it, or wired to a nearby output, would either
    refuse every release or accept a tree from another run. The version is
    asserted alongside because the same step interpolates it into the archive
    names it expects.
    """
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), PUBLISH_JOB)
    upload = named_step(steps, UPLOAD_STEP)
    assert upload.get("uses") == UPLOAD_ACTION, (
        "publication must call the declared-asset action that binds the candidate"
    )
    inputs = require_mapping(upload.get("with"), "upload action inputs")
    assert inputs.get("commit") == CANDIDATE_FORWARD, (
        "the action must receive the validated candidate, not a ref name"
    )
    assert inputs.get("version") == VERSION_FORWARD, (
        "the action must receive the version the archive names interpolate"
    )
