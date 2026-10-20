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
        return _pull_request_coverage_cohort(job)
    return _trunk_cache_writer_cohort(run, job)


def _pull_request_coverage_cohort(job: dict) -> str:
    """Keep Ubicloud PR coverage separate from fork fallback runs."""
    if _has_standard_ubicloud_runner(job):
        return "pr_coverage"
    return "fork_hosted" if "ubuntu-latest" in job.get("labels", []) else "other"


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
