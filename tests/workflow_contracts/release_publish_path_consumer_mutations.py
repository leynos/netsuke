"""Mutate the consumer smoke to prove the release-path contract rejects it.

Each applier names one plausible regression in the smoke's privilege, wiring,
or reachability and applies it to a deep copy. What each regression must break
is stated in ``release_publish_path_consumer_expectations`` and re-exported
here, so the registry and its claims are read together while the claims stay
reviewable on their own.

The accessors that reach into the copied documents live with the shared
mutation types in ``release_publish_path_consumer_mutation_support``, so an
applier reads as the one change it makes rather than as document traversal.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_consumer_artifact_mutations import ARTIFACT_MUTATIONS
from release_publish_path_consumer_expectations import (
    CONSUMER_SMOKE_EXPECTED_VIOLATIONS,
    MUTATION_LABELS,
)
from release_publish_path_consumer_mutation_support import (
    ConsumerSmokeMutationError,
    ConsumerSmokeMutationIssue,
    Job,
    cli_invocation,
    invocation,
    permissions,
    remove_guard_clause,
    smoke_job,
    steps,
)
from release_publish_path_consumer_prologue_mutations import PROLOGUE_MUTATIONS
from release_publish_path_consumer_script import runs_smoke_cli
from release_publish_path_consumer_shape import command_lines

if typ.TYPE_CHECKING:
    import collections.abc as cabc


#: The guard clauses, each with the operator that joins it to the previous
#: clause, so removing one leaves an expression the evaluator can still parse.
CANCELLED_CLAUSE = "!cancelled() && "
FINALIZE_CLAUSE = "needs.finalize-github-release.result == 'success' && "
PUBLISH_CLAUSE = "&& needs.metadata.outputs.should_publish == 'true'"
#: A whole build command. ``cargo install netsuke-build`` is what a smoke that
#: compiled instead of downloading would run, and is the substitution the
#: compilation contract exists to refuse.
COMPILATION_STEP = {"name": "Install from source", "run": "cargo install netsuke-build"}
#: A step that expands the workflow token in its own shell while declaring no
#: environment, so only a scan of executable lines sees the credential.
TOKEN_READING_STEP = {
    "name": "Read the token",
    "run": 'echo "${{ github.token }}" > token.txt',
}
CHECKOUT_PREFIX = "actions/checkout@"


def _drop_finalize_need(release: Job, _action: Job) -> None:
    """Remove finalization from the smoke's dependencies and its guard.

    Both halves go together: a dependency deleted from ``needs`` while the
    guard still reads its result is refused by the expression evaluator rather
    than by the dependency contract this mutation targets.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job declares no dependency list, so the mutation would change
        nothing the contract reads.
    """
    job = smoke_job(release)
    needs = job.get("needs")
    if not isinstance(needs, list):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_NEEDS)
    needs.remove("finalize-github-release")
    remove_guard_clause(job, FINALIZE_CLAUSE)


def _ignore_finalize_result(release: Job, _action: Job) -> None:
    """Stop requiring finalization to have succeeded, keeping the dependency."""
    remove_guard_clause(smoke_job(release), FINALIZE_CLAUSE)


def _remove_publish_guard(release: Job, _action: Job) -> None:
    """Let the smoke run in a mode that publishes nothing."""
    remove_guard_clause(smoke_job(release), PUBLISH_CLAUSE)


def _remove_cancelled_guard(release: Job, _action: Job) -> None:
    """Drop the explicit cancellation check from the smoke's guard."""
    remove_guard_clause(smoke_job(release), CANCELLED_CLAUSE)


def _remove_permissions(release: Job, _action: Job) -> None:
    """Remove the job's permission grant entirely."""
    smoke_job(release).pop("permissions", None)


def _grant_contents_write(release: Job, _action: Job) -> None:
    """Broaden the smoke's repository permission to write."""
    permissions(smoke_job(release))["contents"] = "write"


def _add_id_token(release: Job, _action: Job) -> None:
    """Grant the smoke an OIDC token exchange it must never need."""
    permissions(smoke_job(release))["id-token"] = "write"


def _add_environment(release: Job, _action: Job) -> None:
    """Bind the smoke to a deployment environment's secrets."""
    smoke_job(release)["environment"] = "release"


def _add_token_environment(release: Job, _action: Job) -> None:
    """Give the smoke a GitHub token in its job environment."""
    smoke_job(release)["env"] = {"GH_TOKEN": "${{ github.token }}"}


def _add_workflow_token_environment(release: Job, _action: Job) -> None:
    """Give the whole workflow a token, leaving the smoke job's environment clean.

    A top-level variable reaches every job without appearing in any of them,
    so a scan of the smoke job alone cannot see it.
    """
    release["env"] = {"GH_TOKEN": "${{ github.token }}"}


def _add_action_token_environment(_release: Job, action: Job) -> None:
    """Give one of the action's own steps a GitHub token.

    The job's environment stays clean, so only a scan of the action sees this.

    Raises
    ------
    ConsumerSmokeMutationError
        If no action step carries a command to attach the environment to.
    """
    for step in steps(action):
        if "run" in step:
            step["env"] = {"GITHUB_TOKEN": "${{ github.token }}"}
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def _guard_invocation(release: Job, _action: Job) -> None:
    """Stop the job's one smoke invocation from running.

    Nothing else changes: the dependency list, the job guard, and the inputs
    all still read as correct while the step that exercises the release never
    runs.
    """
    invocation(smoke_job(release))["if"] = "false"


def _persist_checkout_credentials(release: Job, _action: Job) -> None:
    """Leave the workflow token in the job's git configuration.

    This is the mutation the checkout contract exists for: the token is then
    readable by every later step of a job that is supposed to hold none.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job has no checkout step, or its checkout passes no inputs, so
        the mutation would change nothing the contract reads.
    """
    for step in steps(smoke_job(release)):
        if not str(step.get("uses", "")).startswith(CHECKOUT_PREFIX):
            continue
        values = step.get("with")
        if not isinstance(values, dict):
            raise ConsumerSmokeMutationError(
                ConsumerSmokeMutationIssue.MISSING_CHECKOUT_INPUTS
            )
        values["persist-credentials"] = True
        return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_CHECKOUT)


def _absorb_job_failure(release: Job, _action: Job) -> None:
    """Absorb a failed smoke at the job level."""
    smoke_job(release)["continue-on-error"] = True


def _absorb_step_failure(release: Job, _action: Job) -> None:
    """Absorb a failed smoke on its one invocation step."""
    invocation(smoke_job(release))["continue-on-error"] = True


def _absorb_action_failure(_release: Job, action: Job) -> None:
    """Absorb a failed verification inside the composite action.

    The job and its invocation step stay clean, so only a scan of the action's
    own steps sees the failure being swallowed.
    """
    declared, index = cli_invocation(action, "verify")
    declared[index]["continue-on-error"] = True


def _drop_verifier(_release: Job, action: Job) -> None:
    """Remove the action's verify invocation, keeping its report step.

    The report step still emits an outcome for every phase — all ``skipped`` —
    so the run reads as a completed smoke that verified nothing.
    """
    declared, index = cli_invocation(action, "verify")
    del declared[index]


def _echo_verifier(_release: Job, action: Job) -> None:
    """Replace the verifier's launcher with ``echo``, printing its command only.

    Every reviewed argument stays in place, so only a contract that compares
    the whole command line, launcher included, notices that nothing verified.

    Raises
    ------
    ConsumerSmokeMutationError
        If no verify line remains to prefix, so the mutation would change
        nothing the contract reads.
    """
    declared, index = cli_invocation(action, "verify")
    step = declared[index]
    for line in command_lines(step.get("run")):
        if runs_smoke_cli(line):
            step["run"] = f"echo {line}"
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def _read_token_in_job_run(release: Job, _action: Job) -> None:
    """Append a job step that reads the workflow token in its own shell."""
    steps(smoke_job(release)).append(dict(TOKEN_READING_STEP))


def _read_token_in_action_run(_release: Job, action: Job) -> None:
    """Append an action step that reads the workflow token in its own shell."""
    steps(action).append(dict(TOKEN_READING_STEP))


def _compile_in_job(release: Job, _action: Job) -> None:
    """Append the source build a smoke must never run."""
    steps(smoke_job(release)).append(dict(COMPILATION_STEP))


def _compile_in_action(release: Job, action: Job) -> None:
    """Append the source build to the action the job calls."""
    _ = release
    steps(action).append(dict(COMPILATION_STEP))


_MUTATIONS: dict[str, cabc.Callable[[Job, Job], None]] = {
    "drop-consumer-smoke-finalize-prerequisite": _drop_finalize_need,
    "ignore-consumer-smoke-finalize-result": _ignore_finalize_result,
    "remove-consumer-smoke-publish-guard": _remove_publish_guard,
    "remove-consumer-smoke-cancelled-guard": _remove_cancelled_guard,
    "remove-consumer-smoke-permissions": _remove_permissions,
    "grant-consumer-smoke-contents-write": _grant_contents_write,
    "add-consumer-smoke-id-token": _add_id_token,
    "add-consumer-smoke-environment": _add_environment,
    "add-consumer-smoke-token-environment": _add_token_environment,
    "add-consumer-smoke-workflow-token": _add_workflow_token_environment,
    "add-consumer-smoke-action-token-environment": _add_action_token_environment,
    "add-consumer-smoke-invocation-guard": _guard_invocation,
    "persist-consumer-smoke-checkout-credentials": _persist_checkout_credentials,
    **ARTIFACT_MUTATIONS,
    "absorb-consumer-smoke-job-failure": _absorb_job_failure,
    "absorb-consumer-smoke-step-failure": _absorb_step_failure,
    "absorb-consumer-smoke-action-failure": _absorb_action_failure,
    "drop-consumer-smoke-verifier": _drop_verifier,
    "echo-consumer-smoke-verifier": _echo_verifier,
    **PROLOGUE_MUTATIONS,
    "read-consumer-smoke-token-in-run": _read_token_in_job_run,
    "read-consumer-smoke-action-token-in-run": _read_token_in_action_run,
    "compile-in-consumer-smoke-job": _compile_in_job,
    "compile-in-consumer-smoke-action": _compile_in_action,
}


def mutate_consumer_smoke(label: str, release: Job, action: Job) -> None:
    """Apply one named consumer-smoke regression in place.

    Parameters
    ----------
    label
        Name of the registered mutation to apply.
    release
        A deep copy of the release workflow, mutated in place.
    action
        A deep copy of the consumer-smoke manifest, mutated in place.

    Raises
    ------
    ConsumerSmokeMutationError
        If ``label`` is not registered, or if the fixture it targets has moved
        so far that the mutation would change nothing.
    """
    applier = _MUTATIONS.get(label)
    if applier is None:
        raise ConsumerSmokeMutationError(
            ConsumerSmokeMutationIssue.UNKNOWN_LABEL, label
        )
    applier(release, action)


__all__ = [
    "CONSUMER_SMOKE_EXPECTED_VIOLATIONS",
    "MUTATION_LABELS",
    "mutate_consumer_smoke",
]
