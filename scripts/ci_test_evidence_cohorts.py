"""Classify CI records into source and runner-specific observation cohorts.

These labels keep PR coverage, trunk coverage, trusted cache writers, and fork
fallback jobs separate during analysis.
"""

from pathlib import PurePosixPath

from ci_test_evidence_types import validate_identities


def classify_cohort(run: dict, job: dict, pull_request: dict | None = None) -> str:
    """Classify a run and job into a fixed comparison cohort.

    Parameters
    ----------
    run
        Saved GitHub run API response containing workflow path and event data.
    job
        Saved GitHub job API response containing its name and runner labels.
    pull_request
        Optional separately saved GitHub pull request API response. It is used
        only when its identity and base repository match the run association.

    Returns
    -------
    str
        One of the fixed cohort labels; empty inputs return ``"other"``.
    """
    workflow = PurePosixPath(str(run.get("path", "")).split("@", 1)[0]).name
    if workflow == "coverage-main.yml" and job.get("name") == "coverage-upload":
        return _trunk_coverage_cohort(run, job)
    if workflow == "ci.yml" and job.get("name") == "build-test":
        return _ci_coverage_cohort(run, job, pull_request)
    return "other"


def _trunk_coverage_cohort(run: dict, job: dict) -> str:
    """Classify the main-only coverage baseline lane."""
    return (
        "trunk_coverage"
        if _is_trunk_push(run) and _has_standard_ubicloud_runner(job)
        else "other"
    )


def _ci_coverage_cohort(run: dict, job: dict, pull_request: dict | None) -> str:
    """Classify PR coverage, fork fallback, and trunk cache-writer runs."""
    if run.get("event") == "pull_request":
        return _pull_request_coverage_cohort(run, job, pull_request)
    return _trunk_cache_writer_cohort(run, job)


def _pull_request_coverage_cohort(
    run: dict, job: dict, pull_request: dict | None
) -> str:
    """Keep paid, fork, and Dependabot PR lanes separate."""
    if _has_standard_ubicloud_runner(job):
        return "pr_coverage"
    if "ubuntu-latest" not in job.get("labels", []):
        return "other"
    if _is_dependabot_pull_request(run, pull_request):
        return "dependabot_hosted"
    return "fork_hosted" if _has_distinct_head_repository(run) else "other"


def _is_dependabot_pull_request(run: dict, pull_request: dict | None) -> bool:
    """Read a matching saved PR author, not the actor who most recently pushed."""
    if not isinstance(pull_request, dict):
        return False
    pull_requests = run.get("pull_requests")
    if not isinstance(pull_requests, list) or len(pull_requests) != 1:
        return False
    run_pull_request = pull_requests[0]
    if not isinstance(run_pull_request, dict):
        return False
    if not _pull_request_metadata_matches(run, run_pull_request, pull_request):
        return False
    user = pull_request.get("user")
    return isinstance(user, dict) and user.get("login") == "dependabot[bot]"


def _pull_request_metadata_matches(
    run: dict, run_pull_request: dict, pull_request: dict
) -> bool:
    """Bind separately saved PR metadata to its run and base repository."""
    run_repository = run.get("repository")
    api_base = pull_request.get("base")
    api_base_repository = api_base.get("repo") if isinstance(api_base, dict) else None
    if not isinstance(run_repository, dict) or not isinstance(
        api_base_repository, dict
    ):
        return False

    identities = (
        run_pull_request.get("id"),
        pull_request.get("id"),
        run_pull_request.get("number"),
        pull_request.get("number"),
        run_repository.get("id"),
        api_base_repository.get("id"),
    )
    try:
        validate_identities(list(identities))
    except TypeError, ValueError:
        return False

    run_identity = (
        run_pull_request["id"],
        run_pull_request["number"],
        run_repository["id"],
    )
    api_identity = (
        pull_request["id"],
        pull_request["number"],
        api_base_repository["id"],
    )
    return run_identity == api_identity and _run_association_base_matches_repository(
        run_pull_request, run_repository["id"]
    )


def _run_association_base_matches_repository(
    run_pull_request: dict, repository_id: int
) -> bool:
    """Require a supplied run base repository to match its workflow repository."""
    run_base = run_pull_request.get("base")
    if run_base is None:
        return True
    if not isinstance(run_base, dict):
        return False

    run_base_repository = run_base.get("repo")
    if run_base_repository is None:
        return True
    if not isinstance(run_base_repository, dict):
        return False

    base_repository_id = run_base_repository.get("id")
    return _is_repository_id(base_repository_id) and base_repository_id == repository_id


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
