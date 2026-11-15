"""Mutate the consumer smoke to prove the release-path contract rejects it.

Each applier names one plausible regression in the smoke's privilege, wiring,
or reachability and applies it to a deep copy. What each regression must break
is stated in ``release_publish_path_consumer_expectations`` and re-exported
here, so the registry and its claims are read together while the claims stay
reviewable on their own.

Run via ``make test-workflow-contracts``.
"""

import enum
import typing as typ

from release_publish_path_consumer_expectations import (
    CONSUMER_SMOKE_EXPECTED_VIOLATIONS,
    MUTATION_LABELS,
)
from release_publish_path_consumer_smoke import CONSUMER_SMOKE_JOB, LOCAL_SMOKE_ACTION

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One parsed workflow or composite-action document, mutated in place.
type Job = dict[str, object]


class ConsumerSmokeMutationIssue(enum.StrEnum):
    """Classify why a mutation no longer matches the fixture it mutates."""

    MISSING_JOBS = "the release workflow must declare its jobs"
    MISSING_JOB = f"the release workflow must declare {CONSUMER_SMOKE_JOB}"
    MALFORMED_STEPS = "the mutated document must declare a list of step mappings"
    MISSING_CLAUSE = "the smoke guard no longer carries the clause to remove"
    MISSING_PERMISSIONS = f"{CONSUMER_SMOKE_JOB} must set permissions"
    MISSING_NEEDS = f"{CONSUMER_SMOKE_JOB} must declare needs"
    MISSING_CHECKOUT_INPUTS = "the smoke checkout must pass inputs"
    MISSING_CHECKOUT = f"{CONSUMER_SMOKE_JOB} must check the candidate out"
    MISSING_DOWNLOAD = f"{CONSUMER_SMOKE_JOB} must download at least one artefact"
    MISSING_ACTION_STEP = f"{CONSUMER_SMOKE_JOB} must call {LOCAL_SMOKE_ACTION}"
    MALFORMED_ENVIRONMENT = "the mutated step must carry an environment mapping"
    UNKNOWN_LABEL = "unknown consumer-smoke mutation"


class ConsumerSmokeMutationError(AssertionError):
    """Report a mutation that no longer matches the fixture it mutates.

    Raised explicitly rather than left to an ``assert`` so the precondition
    survives ``python -O``, where an assertion is dropped and a mutation would
    silently apply to nothing. The category carries the message and the detail
    carries whatever the category cannot say, which keeps long explanations out
    of the ``raise`` sites.
    """

    def __init__(
        self, issue: ConsumerSmokeMutationIssue, detail: object | None = None
    ) -> None:
        """Record the mutation error category and any stale clause or label."""
        self.issue = issue
        self.detail = detail
        super().__init__(issue, detail)

    def __str__(self) -> str:
        """Render the mutation error with the detail that makes it diagnosable."""
        if self.detail is None:
            return self.issue.value
        return f"{self.issue.value}: {self.detail}"


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
    """Give the smoke a GitHub token in its environment."""
    _smoke_job(release)["env"] = {"GH_TOKEN": "${{ github.token }}"}


def _add_action_token_environment(_release: Job, action: Job) -> None:
    """Give one of the action's own steps a GitHub token.

    The job's environment stays clean, so only a scan of the action can see
    this; that separation is what the mutation exists to prove.

    Raises
    ------
    ConsumerSmokeMutationError
        If no action step carries a command to attach the environment to, or
        the step it does carry has no environment mapping.
    """
    for step in _steps(action):
        if "run" not in step:
            continue
        env = step.setdefault("env", {})
        if not isinstance(env, dict):
            raise ConsumerSmokeMutationError(
                ConsumerSmokeMutationIssue.MALFORMED_ENVIRONMENT
            )
        # `update` with a mapping, as the verify-inventory mutations write
        # theirs: the value is a workflow expression this fixture plants, not a
        # secret, and the mapping form keeps the bandit rule that flags a
        # credential-named assignment from reading it as one.
        env.update({"GITHUB_TOKEN": "${{ github.token }}"})
        return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def _guard_invocation(release: Job, _action: Job) -> None:
    """Stop the job's one smoke invocation from running.

    Nothing else changes: the dependency list, the job guard, and the inputs
    all still read as correct while the step that exercises the release never
    runs.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job no longer invokes the action, so the mutation would change
        nothing the contract reads.
    """
    for step in _steps(_smoke_job(release)):
        if str(step.get("uses", "")) == LOCAL_SMOKE_ACTION:
            step["if"] = "false"
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


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
    "add-consumer-smoke-action-token-environment": _add_action_token_environment,
    "add-consumer-smoke-invocation-guard": _guard_invocation,
    "persist-consumer-smoke-checkout-credentials": _persist_checkout_credentials,
    "select-consumer-smoke-pattern-artifact": _select_pattern_artifact,
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
