"""Share the mutation vocabulary between the release-path mutation modules.

A mutation states one deliberate regression and the diagnostics it must
provoke. The core release-path mutations and the publication-gate mutations
live apart so each stays beside the contract it exercises, but both need the
same failure vocabulary and the same refusal to apply a rule that no longer
matches: a mutation whose pattern silently stops matching would leave the
contract untested while reporting success.

Run via ``make test-workflow-contracts``.
"""

import enum
import re


class MutationIssue(enum.StrEnum):
    """Classify a stale mutation definition or unknown mutation label."""

    PATTERN_MISMATCH = "mutation pattern did not match exactly once"
    UNKNOWN_LABEL = "unknown release-path mutation"
    MISSING_CALLER_EVENT = "caller pull_request.types must include synchronize"


class MutationError(AssertionError):
    """Describe a mutation helper that no longer matches its test fixture."""

    def __init__(self, issue: MutationIssue, detail: object | None = None) -> None:
        """Record the mutation error category and optional detail."""
        self.issue = issue
        self.detail = detail
        super().__init__(issue, detail)

    def __str__(self) -> str:
        """Render the mutation error with any stale pattern or label."""
        if self.detail is None:
            return self.issue.value
        return f"{self.issue.value}: {self.detail}"


def replace_once(value: str, pattern: str, replacement: str) -> str:
    """Replace one required expression shape, refusing stale mutation rules."""
    mutated, count = re.subn(pattern, replacement, value, count=1)
    if count != 1:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, (pattern, count))
    return mutated


__all__ = ("MutationError", "MutationIssue", "replace_once")
