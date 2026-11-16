"""Mutate the consumer smoke to prove the release-path contract rejects it.

Each applier names one plausible regression in the smoke's privilege, wiring,
or reachability and applies it to a deep copy. What each regression must break
is stated in ``release_publish_path_consumer_expectations`` and re-exported
here, so the registry and its claims are read together while the claims stay
reviewable on their own.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_consumer_commands import action_cli_invocation
from release_publish_path_consumer_expectations import (
    CONSUMER_SMOKE_EXPECTED_VIOLATIONS,
    MUTATION_LABELS,
)
from release_publish_path_consumer_mutation_support import (
    ConsumerSmokeMutationError,
    ConsumerSmokeMutationIssue,
    Job,
)
from release_publish_path_consumer_smoke import CONSUMER_SMOKE_JOB, LOCAL_SMOKE_ACTION

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
DOWNLOAD_PREFIX = "actions/download-artifact@"
CHECKOUT_PREFIX = "actions/checkout@"


def _smoke_job(release: Job) -> Job:
    """Return the smoke job of a copied release workflow.

    Returns
    -------
    Job
        The mutable job mapping.

    Raises
    ------
    ConsumerSmokeMutationError
        If the copied workflow no longer declares the job, which means the
        fixture moved and the mutation would test nothing.
    """
    jobs = release.get("jobs")
    if not isinstance(jobs, dict):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_JOBS)
    job = jobs.get(CONSUMER_SMOKE_JOB)
    if not isinstance(job, dict):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_JOB)
    return job


def _steps(document: Job) -> list[Job]:
    """Return a job's or action's step list for structural mutation.

    Returns
    -------
    list[Job]
        The mutable steps, in declaration order, whether the steps hang off a
        job or off a composite action's ``runs`` mapping.

    Raises
    ------
    ConsumerSmokeMutationError
        If the document declares no steps, or declares one that is not a
        mapping.
    """
    runs = document.get("runs")
    holder = runs if isinstance(runs, dict) else document
    declared = holder.get("steps")
    if not isinstance(declared, list) or not all(
        isinstance(step, dict) for step in declared
    ):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MALFORMED_STEPS)
    # The caller mutates this list in place, so it must be the one the document
    # holds. A filtered copy would accept an append and change nothing.
    return typ.cast("list[Job]", declared)


def _without(guard: str, clause: str) -> str:
    """Return ``guard`` with one required clause removed.

    Returns
    -------
    str
        The guard without the clause.

    Raises
    ------
    ConsumerSmokeMutationError
        If the clause is absent, which means the guard was rewritten and this
        mutation no longer removes anything.
    """
    if clause not in guard:
        raise ConsumerSmokeMutationError(
            ConsumerSmokeMutationIssue.MISSING_CLAUSE, (clause, guard)
        )
    return guard.replace(clause, "", 1)


def _set_guard(job: Job, clause: str) -> None:
    """Remove ``clause`` from the smoke job's guard."""
    job["if"] = _without(str(job.get("if", "")), clause)


def _permissions(job: Job) -> Job:
    """Return the job's permission mapping for mutation.

    Returns
    -------
    Job
        The mutable permissions.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job declares no permissions to mutate.
    """
    permissions = job.get("permissions")
    if not isinstance(permissions, dict):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_PERMISSIONS)
    return permissions


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
    job = _smoke_job(release)
    needs = job.get("needs")
    if not isinstance(needs, list):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_NEEDS)
    needs.remove("finalize-github-release")
    _set_guard(job, FINALIZE_CLAUSE)


def _ignore_finalize_result(release: Job, _action: Job) -> None:
    """Stop requiring finalization to have succeeded, keeping the dependency."""
    _set_guard(_smoke_job(release), FINALIZE_CLAUSE)


def _remove_publish_guard(release: Job, _action: Job) -> None:
    """Let the smoke run in a mode that publishes nothing."""
    _set_guard(_smoke_job(release), PUBLISH_CLAUSE)


def _remove_cancelled_guard(release: Job, _action: Job) -> None:
    """Drop the explicit cancellation check from the smoke's guard."""
    _set_guard(_smoke_job(release), CANCELLED_CLAUSE)


def _remove_permissions(release: Job, _action: Job) -> None:
    """Remove the job's permission grant entirely."""
    _smoke_job(release).pop("permissions", None)


def _grant_contents_write(release: Job, _action: Job) -> None:
    """Broaden the smoke's repository permission to write."""
    _permissions(_smoke_job(release))["contents"] = "write"


def _add_id_token(release: Job, _action: Job) -> None:
    """Grant the smoke an OIDC token exchange it must never need."""
    _permissions(_smoke_job(release))["id-token"] = "write"


def _add_environment(release: Job, _action: Job) -> None:
    """Bind the smoke to a deployment environment's secrets."""
    _smoke_job(release)["environment"] = "release"


def _add_token_environment(release: Job, _action: Job) -> None:
    """Give the smoke a GitHub token in its job environment."""
    _smoke_job(release)["env"] = {"GH_TOKEN": "${{ github.token }}"}


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
    for step in _steps(action):
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
    _invocation(_smoke_job(release))["if"] = "false"


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
    for step in _steps(_smoke_job(release)):
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


def _select_pattern_artifact(release: Job, _action: Job) -> None:
    """Replace one exact artefact download with a wildcard pattern download.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job downloads no artefact, so the mutation would change nothing
        the contract reads.
    """
    for step in _steps(_smoke_job(release)):
        if str(step.get("uses", "")).startswith(DOWNLOAD_PREFIX):
            step["with"] = {"pattern": "release-*", "path": "downloaded"}
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_DOWNLOAD)


def _invocation(job: Job) -> Job:
    """Return the job's one smoke invocation step.

    Returns
    -------
    Job
        The mutable step that calls the local action.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job no longer invokes the action, so a mutation targeting that
        step would change nothing the contract reads.
    """
    for step in _steps(job):
        if str(step.get("uses", "")) == LOCAL_SMOKE_ACTION:
            return step
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def _drop_artifact_download(release: Job, _action: Job) -> None:
    """Remove one artefact download, leaving the other exact and in place.

    The surviving download still names its artefact and passes every check made
    of a download that exists, so only a contract that requires both to be
    present notices the evidence the smoke no longer receives.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job downloads no artefact, so the mutation would change nothing
        the contract reads.
    """
    steps = _steps(_smoke_job(release))
    for index, step in enumerate(steps):
        if str(step.get("uses", "")).startswith(DOWNLOAD_PREFIX):
            del steps[index]
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_DOWNLOAD)


def _absorb_job_failure(release: Job, _action: Job) -> None:
    """Absorb a failed smoke at the job level."""
    _smoke_job(release)["continue-on-error"] = True


def _absorb_step_failure(release: Job, _action: Job) -> None:
    """Absorb a failed smoke on its one invocation step."""
    _invocation(_smoke_job(release))["continue-on-error"] = True


def _cli_invocation(action: Job, phase: str) -> tuple[list[Job], int]:
    """Return the action's step list and the index of one CLI invocation.

    Returns
    -------
    tuple[list[Job], int]
        The action's own step list — not a copy, so the caller mutates or
        deletes the step in place — and the index of the step that runs
        ``consumer_smoke_cli.py <phase>``.

    Raises
    ------
    ConsumerSmokeMutationError
        If no step runs that subcommand, so the mutation would change nothing
        the contract reads.
    """
    steps = _steps(action)
    for index, step in enumerate(steps):
        tokens = action_cli_invocation(step)
        if tokens is not None and tokens[1:2] == (phase,):
            return steps, index
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def _absorb_action_failure(_release: Job, action: Job) -> None:
    """Absorb a failed verification inside the composite action.

    The job and its invocation step stay clean, so only a scan of the action's
    own steps sees the failure being swallowed.
    """
    steps, index = _cli_invocation(action, "verify")
    steps[index]["continue-on-error"] = True


def _drop_verifier(_release: Job, action: Job) -> None:
    """Remove the action's verify invocation, keeping its report step.

    The report step still emits an outcome for every phase — all ``skipped`` —
    so the run reads as a completed smoke that verified nothing.
    """
    steps, index = _cli_invocation(action, "verify")
    del steps[index]


def _compile_in_job(release: Job, _action: Job) -> None:
    """Append the source build a smoke must never run."""
    _steps(_smoke_job(release)).append(dict(COMPILATION_STEP))


def _compile_in_action(release: Job, action: Job) -> None:
    """Append the source build to the action the job calls."""
    _ = release
    _steps(action).append(dict(COMPILATION_STEP))


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
    "select-consumer-smoke-pattern-artifact": _select_pattern_artifact,
    "drop-consumer-smoke-artifact-download": _drop_artifact_download,
    "absorb-consumer-smoke-job-failure": _absorb_job_failure,
    "absorb-consumer-smoke-step-failure": _absorb_step_failure,
    "absorb-consumer-smoke-action-failure": _absorb_action_failure,
    "drop-consumer-smoke-verifier": _drop_verifier,
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
