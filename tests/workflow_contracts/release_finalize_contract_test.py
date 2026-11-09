"""Hold the release-finalization action and job to their contract.

Finalization is the one release stage whose effect cannot be undone, so the
properties this suite pins are the ones that keep that effect safe: it runs
only after every producer and the verifier have succeeded, it reconciles
against the trusted pre-upload digest record rather than the verifier's output,
it holds no credential beyond the release token, and it makes exactly one
call that can change anything. Each property is asserted against the checked
documents and then against a deliberate mutation, so the assertion is shown to
notice the change rather than merely to pass today.

Run via ``make test-workflow-contracts``.
"""

import copy
import re

import pytest
from release_finalize_contract_support import (
    ARTIFACT_ACTION,
    CHECKOUT_ACTION,
    EXPECTED_ENVIRONMENTS,
    EXPECTED_INPUTS,
    EXPECTED_JOB_INPUTS,
    EXPECTED_NEEDS,
    EXPECTED_OUTPUTS,
    EXPECTED_PERMISSIONS,
    FINALIZE_COMMAND,
    FINALIZE_JOB,
    FINALIZE_STEP,
    FORBIDDEN_IN_RUN,
    LOCAL_ACTION,
    PLAN_STEP,
    RESOLVE_STEP,
    SETUP_UV_ACTION,
    SNAPSHOT_STEP,
    STEP_NAMES,
    TOKEN_STEPS,
    TRUSTED_ARTIFACT_OUTPUT,
    load_action,
    load_action_steps,
    mutate_action,
    mutate_workflow,
    run_tokens,
)
from release_publish_needs import job_names
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

#: The whole-job ceiling the action's step ceiling must sit beneath. The pinned
#: Cuprum release bounds no call in process, so a timeout is the only thing
#: that stops a hung `gh` from holding the job open.
JOB_TIMEOUT_MINUTES = 30

#: The ceiling on the workflow step that invokes the composite action. It sits
#: here rather than on the action's own steps because the runner does not
#: support `timeout-minutes` inside a composite action (actions/runner#1979),
#: which is why the action bounds each command with GNU `timeout` instead.
FINALIZE_STEP_TIMEOUT_MINUTES = 28

#: The longest per-command bound the action may give one adapter invocation.
#: Four of them, at most, must fit under :data:`FINALIZE_STEP_TIMEOUT_MINUTES`.
MAX_COMMAND_BOUND_SECONDS = 600

#: ``timeout --kill-after=30s 300s`` and the line continuation that keeps the
#: bounded command on the next line, where :func:`run_tokens` finds it.
_COMMAND_BOUND = re.compile(r"timeout --kill-after=\d+s (\d+)s \\")

#: The steps whose ``run`` script invokes an adapter, and so needs a bound. The
#: validate step runs only shell builtins and `mkdir`.
BOUNDED_STEPS = (RESOLVE_STEP, SNAPSHOT_STEP, PLAN_STEP, FINALIZE_STEP)


def _workflow() -> dict[str, object]:
    """Read the release workflow for the finalization job contract."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


#: A whole `inputs['github-token'] != ''` comparison, which yields a boolean
#: rather than the token. Removing it leaves any genuine reference in place.
_PRESENCE_TEST = re.compile(
    r"inputs(?:\[['\"]github-token['\"]\]|\.github-token)\s*[!=]=\s*''"
)


def _receives_token_value(value: object) -> bool:
    """Report whether an environment value resolves to the token itself.

    A comparison against the empty string asks only whether the caller supplied
    one, which is all a validation step needs; any other mention of the input
    hands the credential to a step that has no reason to hold it.

    Returns
    -------
    bool
        ``True`` when the value still names the token input once its presence
        tests are removed.
    """
    return "github-token" in _PRESENCE_TEST.sub("", str(value))


def _assert_step_shape(step: dict[str, object]) -> None:
    """Require strict shell handling, environment-only substitutions, and bounds."""
    name = str(step.get("name"))
    _assert_shell_shape(name, step)
    _assert_step_bounds(name, step)


def _assert_shell_shape(name: str, step: dict[str, object]) -> None:
    """Require the shell and substitution discipline every step must keep."""
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
    for forbidden in FORBIDDEN_IN_RUN:
        assert forbidden not in run, (
            f"{name} must not call {forbidden!r}; the adapter owns that boundary"
        )


def _assert_step_bounds(name: str, step: dict[str, object]) -> None:
    """Require the bound to live where the runner actually honours it."""
    # The runner ignores this key inside a composite action, so a step that
    # carried one would look bounded and be nothing of the sort.
    assert "timeout-minutes" not in step, (
        f"{name} must not carry timeout-minutes: the runner does not support it "
        "in a composite action (actions/runner#1979)"
    )
    if name in BOUNDED_STEPS:
        _assert_command_bound(name, str(step.get("run")))


def _assert_command_bound(name: str, run: str) -> None:
    """Require each adapter invocation to run under a shell-level bound."""
    lines = run.splitlines()
    start = next(index for index, line in enumerate(lines) if "uv run " in line)
    assert start > 0, f"{name} must invoke the adapter on its own command line"
    bound = _COMMAND_BOUND.fullmatch(lines[start - 1].strip())
    assert bound, f"{name} must bound its adapter invocation with `timeout`"
    assert 0 < int(bound.group(1)) <= MAX_COMMAND_BOUND_SECONDS, (
        f"{name} must bound its adapter invocation within "
        f"{MAX_COMMAND_BOUND_SECONDS} seconds, got {bound.group(1)}"
    )


def _assert_action_contract(action: dict[str, object]) -> None:
    """Check the action's interface, token routing, and single mutating call."""
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
    for step in steps:
        name = str(step.get("name"))
        _assert_step_shape(step)
        environment = require_mapping(step.get("env"), f"{name} environment")
        # The token rule is asserted before the environment comparison, and
        # independently of it: the expected-environment map is what recorded
        # the leak as correct in the first place — the validate step held the
        # credential only to test that it was non-empty — so a rule that only
        # restated that map could not catch the same mistake twice.
        if name not in TOKEN_STEPS:
            leaked = sorted(
                variable
                for variable, value in environment.items()
                if _receives_token_value(value)
            )
            assert not leaked, (
                f"{name} must not forward the token expression under any name; "
                f"only {TOKEN_STEPS} reach GitHub, got {leaked}"
            )
        assert environment == EXPECTED_ENVIRONMENTS[name], (
            f"{name} must forward exactly its contracted environment names"
        )
        assert ("GH_TOKEN" in environment) is (name in TOKEN_STEPS), (
            f"{name} must hold a token only when it must reach GitHub"
        )

    finalize = named_step(steps, FINALIZE_STEP)
    run = str(finalize.get("run"))
    assert run_tokens(run) == FINALIZE_COMMAND, (
        "the finalize step must run exactly the contracted adapter command"
    )
    # Leaving the release a draft on a refusal is implemented by not
    # swallowing the adapter's exit status: any of these forms would turn a
    # refusal into a green step that published nothing.
    assert "continue-on-error" not in finalize, (
        "the finalize step must not continue past a refused publication"
    )
    assert "set +e" not in run, (
        "the finalize step must let the adapter's exit status fail the step"
    )
    assert "|| " not in run, "the finalize step must not fall back after a refusal"


def _assert_job_guard(job: dict[str, object]) -> None:
    """Require the dependency list, success guard, and privilege of the job."""
    needs = job_names(job.get("needs"))
    assert needs is not None, (
        "the job's needs must be a job name or a list of job names, got "
        f"{job.get('needs')!r}"
    )
    assert sorted(needs) == sorted(EXPECTED_NEEDS), (
        f"the job must depend on exactly {EXPECTED_NEEDS}, got {needs!r}"
    )
    assert len(needs) == len(EXPECTED_NEEDS), (
        "a repeated dependency is an error GitHub rejects, not an extra witness"
    )
    guard = str(job.get("if"))
    assert "!cancelled()" in guard, "the job must keep its explicit cancellation guard"
    assert "needs.metadata.outputs.should_publish == 'true'" in guard, (
        "a rehearsal must never finalize a release"
    )
    for name in EXPECTED_NEEDS:
        if name == "metadata":
            continue
        assert f"needs.{name}.result == 'success'" in guard, (
            f"the guard must name {name}'s success; `!cancelled()` suppresses "
            "the implicit success check"
        )
    permissions = require_mapping(job.get("permissions"), "finalize permissions")
    assert permissions == EXPECTED_PERMISSIONS, (
        "finalization needs the release token and nothing else"
    )
    timeout = job.get("timeout-minutes")
    assert isinstance(timeout, int), "the job must carry a timeout-minutes"
    assert timeout == JOB_TIMEOUT_MINUTES, (
        f"the job's ceiling must stay {JOB_TIMEOUT_MINUTES} minutes, got {timeout}"
    )


def _assert_job_contract(workflow: dict[str, object]) -> None:
    """Check the job's guard, permissions, artefact trust, and local action."""
    _assert_job_guard(workflow_job(workflow, FINALIZE_JOB))
    steps = job_steps(workflow, FINALIZE_JOB)
    checkout = steps[step_index_by_key(steps, "uses", CHECKOUT_ACTION)]
    checkout_with = require_mapping(checkout.get("with"), "checkout inputs")
    assert checkout_with.get("persist-credentials") is False, (
        "the candidate checkout must not persist credentials"
    )
    assert checkout_with.get("ref") == (
        "${{ needs.metadata.outputs.candidate_commit }}"
    ), "the job must finalize with the candidate's own scripts"

    setup_uv = steps[step_index_by_key(steps, "uses", SETUP_UV_ACTION)]
    uv_with = require_mapping(setup_uv.get("with"), "setup-uv inputs")
    assert uv_with.get("python-version") == "3.14", (
        "the release helpers run on the pinned baseline interpreter"
    )

    downloads = [
        require_mapping(step.get("with"), "artifact inputs")
        for step in steps
        if str(step.get("uses", "")).startswith(ARTIFACT_ACTION)
    ]
    trusted = [values for values in downloads if values.get("path") == "trusted"]
    assert len(trusted) == 1, "exactly one download must feed the trusted record"
    assert trusted[0].get("name") == TRUSTED_ARTIFACT_OUTPUT, (
        "the digest record must be the artefact the publisher recorded, not one "
        "recomputed from this job's attempt"
    )
    assert "pattern" not in trusted[0], (
        "a pattern could select another run's digest record"
    )
    package = [values for values in downloads if values.get("path") != "trusted"]
    assert len(package) == 1, "exactly one download must feed the package record"
    assert package[0].get("name") == "release-crate-package", (
        "the package record must come from the packaging stage's artefact"
    )
    assert "pattern" not in package[0], (
        "a pattern could select another run's package record"
    )

    local = steps[step_index_by_key(steps, "uses", LOCAL_ACTION)]
    _assert_local_action(local)
    assert steps.index(local) > max(steps.index(checkout), steps.index(setup_uv)), (
        "the finalization action must run after its checkout and tools"
    )


def _assert_local_action(step: dict[str, object]) -> None:
    """Require the action's inputs and the aggregate ceiling around them."""
    inputs = require_mapping(step.get("with"), "finalizer inputs")
    assert inputs == EXPECTED_JOB_INPUTS, (
        "the job must forward the contracted finalization inputs"
    )
    assert inputs["digest-record-path"] == "trusted/trusted.json", (
        "reconciliation must read the trusted pre-upload record; the verifier's "
        "output agrees with GitHub by construction"
    )
    timeout = step.get("timeout-minutes")
    assert isinstance(timeout, int), (
        "the step that invokes the action must carry the aggregate ceiling, "
        "because the action's own steps cannot"
    )
    assert timeout == FINALIZE_STEP_TIMEOUT_MINUTES, (
        f"the action's ceiling must stay {FINALIZE_STEP_TIMEOUT_MINUTES} minutes, "
        f"got {timeout!r}"
    )
    assert timeout < JOB_TIMEOUT_MINUTES, (
        "the action's ceiling must leave the job's own timeout room to report it"
    )


def _assert_complete_contract(
    action: dict[str, object], workflow: dict[str, object]
) -> None:
    """Require the action and the job that calls it to agree."""
    _assert_action_contract(action)
    _assert_job_contract(workflow)


@pytest.mark.parametrize(
    "mutation",
    [
        "token-on-validate",
        "token-on-plan",
        "unbounded-command",
        "gh-in-run",
        "wrong-output",
    ],
)
def test_action_mutations_are_rejected(mutation: str) -> None:
    """Prove the action contract detects each finalizer mutation."""
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
        "missing-verifier",
        "artifact-pattern",
        "trusted-literal",
        "missing-step-timeout",
        "verifier-record",
    ],
)
def test_workflow_mutations_are_rejected(mutation: str) -> None:
    """Prove the job contract detects each dependency and trust mutation."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_workflow(workflow, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_contract(action, workflow)


def test_the_action_and_job_satisfy_the_complete_contract() -> None:
    """Require the checked-in action and workflow to agree on the contract."""
    _assert_complete_contract(load_action(), _workflow())
