"""Specify the source evidence required for hosted PR cohorts."""

import pytest
from ci_test_evidence_cohorts import classify_cohort


@pytest.mark.parametrize(
    ("author", "base_id", "head_id", "expected"),
    [
        ("dependabot[bot]", 42, 42, "dependabot_hosted"),
        ("dependabot[bot]", 42, 99, "dependabot_hosted"),
        ("leynos", 42, 99, "fork_hosted"),
        ("leynos", 42, 42, "other"),
        ("leynos", True, False, "other"),
        (None, 42, 99, "fork_hosted"),
        ("leynos", None, None, "other"),
    ],
    ids=[
        "dependabot-same-repository",
        "dependabot-fork-source",
        "fork-pull-request",
        "same-repository-hosted-run-is-unknown",
        "boolean-values-are-not-repository-IDs",
        "fork-without-pr-author",
        "missing-repository-identities",
    ],
)
def test_hosted_pr_cohort_uses_author_and_repository_identity(
    author: str | None, base_id: int | None, head_id: int | None, expected: str
) -> None:
    """Separate hosted lanes using the saved PR author and repository IDs."""
    pull_requests = [] if author is None else [{"user": {"login": author}}]
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "actor": {"login": "leynos"},
        "pull_requests": pull_requests,
        "repository": {"id": base_id},
        "head_repository": {"id": head_id},
    }
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job) == expected, (
        "Hosted cohorts must use the PR author and head repository provenance"
    )


def test_repository_names_fallback_when_ids_are_unavailable() -> None:
    """Compare repository names when saved numeric identities are missing."""
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "pull_requests": [{"user": {"login": "leynos"}}],
        "repository": {"full_name": "leynos/netsuke"},
        "head_repository": {"full_name": "contributor/netsuke"},
    }
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job) == "fork_hosted", (
        "Distinct saved repository names must identify the hosted fork path"
    )


@pytest.mark.parametrize(
    "pull_requests",
    [None, "unknown", [{}], [{"user": {"login": "dependabot[bot]"}}] * 2],
    ids=["missing", "wrong-type", "missing-author", "ambiguous-authors"],
)
def test_ambiguous_dependabot_identity_is_not_assumed(
    pull_requests: object,
) -> None:
    """Leave hosted runs unclassified when PR author evidence is ambiguous."""
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "pull_requests": pull_requests,
        "repository": {"id": 42},
        "head_repository": {"id": 42},
    }
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job) == "other", (
        "Missing or ambiguous PR metadata must not be labelled as Dependabot"
    )
