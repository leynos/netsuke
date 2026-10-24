"""Classify CI records into source and runner-specific observation cohorts.

These labels keep PR coverage, trunk coverage, trusted cache writers, and fork
fallback jobs separate during analysis.
"""

from pathlib import PurePosixPath


def classify_cohort(run: dict, job: dict) -> str:
    """Classify a run and job into a fixed comparison cohort.

    Parameters
    ----------
    run
        Saved GitHub run API response containing workflow path and event data.
    job
        Saved GitHub job API response containing its name and runner labels.

    Returns
    -------
    str
        One of the fixed cohort labels; empty inputs return ``"other"``.
    """
    workflow = PurePosixPath(str(run.get("path", "")).split("@", 1)[0]).name
    classifier = {
        ("coverage-main.yml", "coverage-upload"): _trunk_coverage_cohort,
        ("ci.yml", "build-test"): _ci_coverage_cohort,
    }.get((workflow, job.get("name")))
    return classifier(run, job) if classifier else "other"


def _trunk_coverage_cohort(run: dict, job: dict) -> str:
    """Classify the main-only coverage baseline lane."""
    return (
        "trunk_coverage"
        if _is_trunk_push(run) and _has_standard_ubicloud_runner(job)
        else "other"
    )


def _ci_coverage_cohort(run: dict, job: dict) -> str:
    """Classify PR coverage, fork fallback, and trunk cache-writer runs."""
    if run.get("event") == "pull_request":
        return _pull_request_coverage_cohort(run, job)
    return _trunk_cache_writer_cohort(run, job)


def _pull_request_coverage_cohort(run: dict, job: dict) -> str:
    """Keep paid, fork, and Dependabot PR lanes separate."""
    if _has_standard_ubicloud_runner(job):
        return "pr_coverage"
    if "ubuntu-latest" not in job.get("labels", []):
        return "other"
    if _is_dependabot_pull_request(run):
        return "dependabot_hosted"
    return "fork_hosted" if _has_distinct_head_repository(run) else "other"


def _is_dependabot_pull_request(run: dict) -> bool:
    """Read the PR author, not the actor who most recently pushed."""
    pull_requests = run.get("pull_requests")
    if not isinstance(pull_requests, list) or len(pull_requests) != 1:
        return False
    pull_request = pull_requests[0]
    if not isinstance(pull_request, dict):
        return False
    user = pull_request.get("user")
    return isinstance(user, dict) and user.get("login") == "dependabot[bot]"


def _has_distinct_head_repository(run: dict) -> bool:
    """Require saved base/head repository identities before calling a PR a fork."""
    base = run.get("repository")
    head = run.get("head_repository")
    if not isinstance(base, dict) or not isinstance(head, dict):
        return False
    base_id, head_id = base.get("id"), head.get("id")
    if _is_repository_id(base_id) and _is_repository_id(head_id):
        return base_id != head_id
    base_name, head_name = base.get("full_name"), head.get("full_name")
    if isinstance(base_name, str) and isinstance(head_name, str):
        return base_name.casefold() != head_name.casefold()
    return False


def _is_repository_id(value: object) -> bool:
    """Accept numeric repository IDs without treating booleans as IDs."""
    return isinstance(value, int) and not isinstance(value, bool)


def _trunk_cache_writer_cohort(run: dict, job: dict) -> str:
    """Identify only trusted trunk pushes that can publish cache archives."""
    return (
        "trunk_cache_writer"
        if _is_trunk_push(run) and _has_standard_ubicloud_runner(job)
        else "other"
    )


def _is_trunk_push(run: dict) -> bool:
    """Return whether a workflow run is a push to the main branch."""
    return run.get("event") == "push" and run.get("head_branch") == "main"


def _has_standard_ubicloud_runner(job: dict) -> bool:
    """Match the four-vCPU runner used by the trusted Linux gate."""
    return "ubicloud-standard-4-ubuntu-2404" in job.get("labels", [])
