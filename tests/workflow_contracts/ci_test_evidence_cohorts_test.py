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
    """Separate hosted lanes using matched PR API and repository identities."""
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "actor": {"login": "leynos"},
        "pull_requests": [
            {
                "id": 9001,
                "number": 17,
                "base": {"repo": {"id": base_id}},
                "head": {"repo": {"id": head_id}},
            }
        ],
        "repository": {"id": base_id},
        "head_repository": {"id": head_id},
    }
    pull_request = (
        None
        if author is None
        else {
            "id": 9001,
            "number": 17,
            "base": {"repo": {"id": base_id}},
            "user": {"login": author},
        }
    )
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job, pull_request) == expected, (
        "Hosted cohorts must use the PR author and head repository provenance"
    )


def test_repository_names_fallback_when_ids_are_unavailable() -> None:
    """Compare repository names when saved numeric identities are missing."""
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "pull_requests": [{"id": 9001, "number": 17}],
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
        "actor": {"login": "dependabot[bot]"},
        "pull_requests": pull_requests,
        "repository": {"id": 42},
        "head_repository": {"id": 42},
    }
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job) == "other", (
        "Missing or ambiguous PR metadata must not be labelled as Dependabot"
    )


@pytest.mark.parametrize(
    ("identity", "expected"),
    [
        ("id", 9002),
        ("number", 18),
        ("base_repository", 99),
        ("run_base_repository", 99),
    ],
    ids=[
        "different-pr-id",
        "different-pr-number",
        "different-base-repository",
        "conflicting-run-base",
    ],
)
def test_dependabot_author_requires_matching_saved_pr_identity(
    identity: str, expected: int
) -> None:
    """Ignore PR author metadata unless its run and repository binding matches."""
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "actor": {"login": "dependabot[bot]"},
        "pull_requests": [
            {
                "id": 9001,
                "number": 17,
                "base": {"repo": {"id": 42}},
            }
        ],
        "repository": {"id": 42},
        "head_repository": {"id": 42},
    }
    pull_request = {
        "id": 9001,
        "number": 17,
        "base": {"repo": {"id": 42}},
        "user": {"login": "dependabot[bot]"},
    }
    match identity:
        case "id":
            pull_request["id"] = expected
        case "number":
            pull_request["number"] = expected
        case "base_repository":
            pull_request["base"]["repo"]["id"] = expected
        case "run_base_repository":
            run["pull_requests"][0]["base"]["repo"]["id"] = expected
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job, pull_request) == "other", (
        "Unmatched PR metadata must not classify the run by its author's name"
    )


@pytest.mark.parametrize(
    ("run_base", "expected"),
    [
        (None, "dependabot_hosted"),
        ({}, "dependabot_hosted"),
        ({"repo": None}, "dependabot_hosted"),
        ("malformed", "other"),
        ({"repo": "malformed"}, "other"),
        ({"repo": {"id": 99}}, "other"),
        ({"repo": {"id": True}}, "other"),
    ],
    ids=[
        "missing-run-base",
        "missing-run-base-repository",
        "null-run-base-repository",
        "malformed-run-base",
        "malformed-run-base-repository",
        "different-run-base-repository",
        "boolean-run-base-repository-id",
    ],
)
def test_run_association_base_must_match_when_present(
    run_base: object, expected: str
) -> None:
    """Use the run repository only when its optional base data is absent."""
    run = {
        "event": "pull_request",
        "path": ".github/workflows/ci.yml",
        "pull_requests": [
            {
                "id": 9001,
                "number": 17,
                "base": run_base,
                "head": {"repo": {"id": 42}},
            }
        ],
        "repository": {"id": 42},
        "head_repository": {"id": 42},
    }
    pull_request = {
        "id": 9001,
        "number": 17,
        "base": {"repo": {"id": 42}},
        "user": {"login": "dependabot[bot]"},
    }
    job = {"name": "build-test", "labels": ["ubuntu-latest"]}

    assert classify_cohort(run, job, pull_request) == expected, (
        "Malformed or conflicting run base data must not classify by author"
    )
