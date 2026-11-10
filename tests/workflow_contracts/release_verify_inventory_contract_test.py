"""Hold the release-inventory verifier action and job to their contract.

The verifier is the step that says the release GitHub holds is the release the
publisher built. Its safety rests on three properties this suite pins: it only
reads, it trusts only the artefact this run produced, and it receives a token
only on the steps that must reach GitHub. Each is asserted against the checked
documents and then against a deliberate mutation, so the assertion is shown to
notice the change rather than merely to pass today.

Run via ``make test-workflow-contracts``.
"""

import copy
import typing as typ

import pytest
from release_publish_path_scenarios import SCENARIOS
from release_publish_path_verification import check_verify_inventory
from release_verify_inventory_mutations import mutate_action, mutate_workflow
from release_verify_inventory_support import (
    ARTIFACT_ACTION,
    CHECKOUT_ACTION,
    EXPECTED_ENVIRONMENTS,
    EXPECTED_INPUTS,
    EXPECTED_JOB_INPUTS,
    EXPECTED_OUTPUTS,
    GH_RELEASE_CALL,
    LOCAL_ACTION,
    NETWORK_STEPS,
    PUBLISH_JOB,
    SETUP_UV_ACTION,
    STEP_NAMES,
    TRUSTED_ARTIFACT_OUTPUT,
    TRUSTED_ARTIFACT_VALUE,
    VERIFY_COMMAND,
    VERIFY_JOB,
    VERIFY_STEP,
    forbidden_gh_flags,
    gh_api_invocations,
    load_action,
    load_action_steps,
    reject_secrets,
    retained_artifact_name,
    run_tokens,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario

#: The metadata outputs a publishing run declares. Only ``should_publish``
#: reaches the verification guard, but the context is filled as a real run's
#: would be so an expression that reads another output still resolves.
PUBLISH_OUTPUTS = {
    "should_publish": "true",
    "dry_run": "false",
    "should_upload_workflow_artifacts": "true",
}


def _workflow() -> dict[str, object]:
    """Read the release workflow for the verification job contract."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _tag_publish() -> Scenario:
    """Return the scenario in which both publishers succeed."""
    return next(scenario for scenario in SCENARIOS if scenario.name == "tag-publish")


def _assert_gh_api_calls(steps: list[dict[str, object]]) -> None:
    """Require read-only, GET-only API calls, and only on the network steps."""
    found = 0
    for step in steps:
        name = str(step.get("name"))
        run = str(step.get("run"))
        assert not GH_RELEASE_CALL.search(run), (
            f"{name} must not use the release subcommand: it can write"
        )
        invocations = gh_api_invocations(run)
        expected = 1 if name in NETWORK_STEPS else 0
        assert len(invocations) == expected, (
            f"{name} must make exactly {expected} gh api call(s), got "
            f"{len(invocations)}"
        )
        for tokens in invocations:
            assert tokens[:4] == ["gh", "api", "--method", "GET"], (
                f"{name} must issue an explicit GET, got {tokens[:4]!r}"
            )
            offending = forbidden_gh_flags(tokens)
            assert not offending, (
                f"{name} must stay read-only; {offending!r} implies a write"
            )
            found += 1
    assert found == len(NETWORK_STEPS), (
        "the scan must find one call per network step, or it proves nothing"
    )


def _assert_verify_step(steps: list[dict[str, object]]) -> None:
    """Require the exact tokenless four-argument verifier invocation."""
    verify = named_step(steps, VERIFY_STEP)
    assert verify.get("id") == "verify", (
        "the verify step id must stay stable for the action output"
    )
    run = str(verify.get("run"))
    assert run_tokens(run) == VERIFY_COMMAND, (
        "the verifier must receive exactly its directory, declaration, trusted "
        "record, and output"
    )
    assert "verified-record-path=" in run, (
        "the step must publish the record path as its action output"
    )
    assert "GH_TOKEN" not in require_mapping(verify.get("env"), "verify environment"), (
        "the verifier reads only downloaded bytes, so it must receive no token"
    )


def _assert_action_contract(action: dict[str, object]) -> None:
    """Check the action's interface, token routing, and read-only commands."""
    inputs = require_mapping(action.get("inputs"), "action inputs")
    assert set(inputs) == set(EXPECTED_INPUTS), (
        "the action must expose exactly the contracted inputs"
    )
    for name, required in EXPECTED_INPUTS.items():
        spec = require_mapping(inputs[name], f"{name} input")
        assert spec.get("required") is required, f"{name} required flag changed"

    outputs = require_mapping(action.get("outputs"), "action outputs")
    assert set(outputs) == set(EXPECTED_OUTPUTS), (
        "the action must expose exactly the contracted outputs"
    )
    for name, value in EXPECTED_OUTPUTS.items():
        output = require_mapping(outputs[name], f"{name} output")
        assert output.get("value") == value, f"{name} output mapping changed"

    steps = load_action_steps(action)
    assert [step.get("name") for step in steps] == list(STEP_NAMES), (
        "the action's steps must keep their contracted order"
    )
    reject_secrets(action)
    for step in steps:
        name = str(step.get("name"))
        assert step.get("shell") == "bash", f"{name} must run under Bash"
        run = step.get("run")
        assert isinstance(run, str), f"{name} must be a shell script"
        assert run.splitlines()[0] == "set -euo pipefail", (
            f"{name} must enable strict shell handling"
        )
        assert "${{" not in run, (
            f"{name} must receive expressions through env, where they are "
            "evaluated once rather than by the shell"
        )
        environment = require_mapping(step.get("env"), f"{name} environment")
        assert environment == EXPECTED_ENVIRONMENTS[name], (
            f"{name} must forward exactly its contracted environment names"
        )
    _assert_gh_api_calls(steps)
    _assert_verify_step(steps)


def _assert_disjoint_inputs(inputs: dict[str, object]) -> None:
    """Require trusted inputs inside their root and disjoint from the rest.

    The action re-checks this at run time, but a caller that names a download
    path as a trusted input has already defeated the reason the verification
    exists: the downloaded bytes would be compared with themselves.
    """
    trusted = str(inputs["trusted-directory"]).rstrip("/")
    for key in ("declaration-file", "trusted-record-file"):
        value = str(inputs[key])
        assert value.startswith(f"{trusted}/"), (
            f"{key} ({value}) must be inside trusted-directory ({trusted})"
        )
    download = str(inputs["download-directory"]).rstrip("/")
    output = str(inputs["output-path"]).rstrip("/")
    for label, value in (("download-directory", download), ("output-path", output)):
        assert not value.startswith(f"{trusted}/"), (
            f"{label} ({value}) must be disjoint from trusted-directory"
        )
        assert not trusted.startswith(f"{value}/"), (
            f"trusted-directory must not sit inside {label} ({value})"
        )
    assert not output.startswith(f"{download}/"), (
        "the output record must not be written into the download directory"
    )
    assert not download.startswith(f"{output}/"), (
        "the download directory must not sit inside the output record's directory"
    )


def _assert_job_contract(workflow: dict[str, object]) -> None:
    """Check the job's steps, trusted artefact, and local-action inputs."""
    job = workflow_job(workflow, VERIFY_JOB)
    steps = job_steps(workflow, VERIFY_JOB)

    checkout = steps[step_index_by_key(steps, "uses", CHECKOUT_ACTION)]
    checkout_with = require_mapping(checkout.get("with"), "checkout inputs")
    assert checkout_with.get("persist-credentials") is False, (
        "the candidate checkout must not persist credentials"
    )
    assert checkout_with.get("ref") == (
        "${{ needs.metadata.outputs.candidate_commit }}"
    ), "the job must verify with the candidate's own scripts"

    setup_uv = steps[step_index_by_key(steps, "uses", SETUP_UV_ACTION)]
    uv_with = require_mapping(setup_uv.get("with"), "setup-uv inputs")
    assert uv_with.get("python-version") == "3.14", (
        "the release helpers run on the pinned baseline interpreter"
    )

    download = steps[step_index_by_key(steps, "uses", ARTIFACT_ACTION)]
    download_with = require_mapping(download.get("with"), "artifact inputs")
    assert download_with.get("name") == TRUSTED_ARTIFACT_OUTPUT, (
        "the job must download the artefact the publisher recorded, not one "
        "recomputed from this job's attempt; a failed-jobs re-run reuses the "
        "publisher without running it again"
    )
    assert "pattern" not in download_with, (
        "a pattern could select another run's declaration or digest record"
    )
    assert download_with.get("path") == "trusted", (
        "the trusted artefact must land in the directory the action names"
    )

    local = steps[step_index_by_key(steps, "uses", LOCAL_ACTION)]
    assert local.get("uses") == LOCAL_ACTION, (
        "the job must use the repository's verification action"
    )
    inputs = require_mapping(local.get("with"), "verifier inputs")
    assert inputs == EXPECTED_JOB_INPUTS, (
        "the job must forward the contracted verification inputs"
    )
    _assert_disjoint_inputs(inputs)
    assert steps.index(local) > max(
        steps.index(checkout), steps.index(setup_uv), steps.index(download)
    ), "the verification action must run after its checkout, tools, and artefact"
    reject_secrets(job)


def _assert_publisher_records_the_artifact_name(workflow: dict[str, object]) -> None:
    """Require the publisher's output to name the artefact it really retained.

    The output is what survives a "Re-run failed jobs", so it has to carry the
    name of the attempt that uploaded the record. Deriving the expected value
    from the upload action keeps the two manifests from drifting apart: a
    changed retention name would otherwise leave the verifier downloading a
    path that nothing writes.
    """
    publisher = workflow_job(workflow, PUBLISH_JOB)
    outputs = require_mapping(publisher.get("outputs"), "publisher outputs")
    assert outputs.get("trusted-artifact") == TRUSTED_ARTIFACT_VALUE, (
        "the publisher must record the retained artefact's name as an output"
    )
    retained = retained_artifact_name()
    assert retained.replace("${{ github.job }}", PUBLISH_JOB) == (
        TRUSTED_ARTIFACT_VALUE
    ), (
        "the publisher's output must match the name the upload action retains "
        f"its record under, got {retained!r}"
    )


def _assert_complete_contract(
    action: dict[str, object], workflow: dict[str, object]
) -> None:
    """Require the action, the job, and the job's reachability to agree."""
    _assert_action_contract(action)
    _assert_job_contract(workflow)
    _assert_publisher_records_the_artifact_name(workflow)
    violations: list[str] = []
    check_verify_inventory(workflow, _tag_publish(), PUBLISH_OUTPUTS, violations)
    assert not violations, f"verify-job reachability contract: {violations}"


@pytest.mark.parametrize(
    "mutation",
    [
        "gh-release-download",
        "gh-release-spacing",
        "gh-api-field",
        "gh-api-inline-field",
        "gh-api-short-field",
        "gh-api-input",
        "gh-api-method-override",
        "gh-api-short-method",
        "gh-api-substitution",
        "secret-token",
    ],
)
def test_action_mutations_are_rejected(mutation: str) -> None:
    """Prove the action contract detects each verifier mutation."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_action(action, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_contract(action, workflow)


@pytest.mark.parametrize(
    "mutation",
    [
        "id-token",
        "release-dependency",
        "trusted-in-download",
        "artifact-pattern",
        "artifact-literal",
        "publisher-output",
    ],
)
def test_workflow_mutations_are_rejected(mutation: str) -> None:
    """Prove the job contract detects each dependency and trust mutation."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_workflow(workflow, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_contract(action, workflow)


@pytest.mark.parametrize(
    "reference",
    [
        "${{ secrets.CARGO_REGISTRY_TOKEN }}",
        "${{ secrets['CARGO_REGISTRY_TOKEN'] }}",
        "${{ toJSON(secrets) }}",
    ],
)
def test_the_secret_scan_reaches_nested_values(reference: str) -> None:
    """Reject a secret reference however it is spelled and however deeply nested.

    The control for :func:`reject_secrets`. Without it the scan could be
    vacuous — checking only top-level keys — and every assertion calling it
    would still pass. The bracketed and whole-context spellings are the ones a
    dotted-substring check would let through.
    """
    with pytest.raises(AssertionError):
        reject_secrets({"steps": [{"env": {"TOKEN": reference}}]})


def test_a_call_inside_a_substitution_is_still_scanned() -> None:
    """Find a write hidden in ``$(...)``, which a line-anchored scan misses.

    The mutation case above shows the contract rejects the shape; this one
    shows why. The scan has to locate the call wherever it sits on the line,
    and the flag check has to see the ``-f`` that turns it into a POST.
    """
    script = 'body=$(gh api --method GET -f x=y "repos/o/r/releases")\n'
    (tokens,) = gh_api_invocations(script)
    assert forbidden_gh_flags(tokens) == ["-f"], tokens


def test_the_action_and_job_satisfy_the_complete_contract() -> None:
    """Require the checked-in action and workflow to agree on the contract."""
    _assert_complete_contract(load_action(), _workflow())
