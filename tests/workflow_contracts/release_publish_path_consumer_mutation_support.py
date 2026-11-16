"""Hold the types every consumer-smoke mutation shares.

The appliers live in ``release_publish_path_consumer_mutations``. This module
carries only what they all speak: the mutable document type and the error a
mutation raises when the fixture it targets has moved. Keeping them apart lets
a second group of appliers be added without either module outgrowing the
repository's file-size bound, and keeps the failure a stale mutation raises
identical wherever it is raised.

Run via ``make test-workflow-contracts``.
"""

import enum

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


__all__ = ["ConsumerSmokeMutationError", "ConsumerSmokeMutationIssue", "Job"]
