"""Hold the accessors every consumer-smoke mutation shares.

The appliers live in ``release_publish_path_consumer_mutations``. This module
carries what they all speak: the mutable document type, the error a mutation
raises when the fixture it targets has moved, and the accessors that reach a
job's or an action's steps and the one invocation each contract names. Keeping
them apart lets a second group of appliers be added without either module
outgrowing the repository's file-size bound, and keeps the failure a stale
mutation raises identical wherever it is raised.

Run via ``make test-workflow-contracts``.
"""

import enum
import typing as typ

from release_publish_path_consumer_script import invoked_phase, smoke_command_line
from release_publish_path_consumer_smoke import CONSUMER_SMOKE_JOB, LOCAL_SMOKE_ACTION

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


def smoke_job(release: Job) -> Job:
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


def steps(document: Job) -> list[Job]:
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


def without_clause(guard: str, clause: str) -> str:
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


def remove_guard_clause(job: Job, clause: str) -> None:
    """Remove ``clause`` from the smoke job's guard."""
    job["if"] = without_clause(str(job.get("if", "")), clause)


def permissions(job: Job) -> Job:
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
    values = job.get("permissions")
    if not isinstance(values, dict):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_PERMISSIONS)
    return values


def invocation(job: Job) -> Job:
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
    for step in steps(job):
        if str(step.get("uses", "")) == LOCAL_SMOKE_ACTION:
            return step
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


def cli_invocation(action: Job, phase: str) -> tuple[list[Job], int]:
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
    declared = steps(action)
    for index, step in enumerate(declared):
        if invoked_phase(smoke_command_line(step)) == phase:
            return declared, index
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_ACTION_STEP)


__all__ = [
    "ConsumerSmokeMutationError",
    "ConsumerSmokeMutationIssue",
    "Job",
    "cli_invocation",
    "invocation",
    "permissions",
    "remove_guard_clause",
    "smoke_job",
    "steps",
    "without_clause",
]
