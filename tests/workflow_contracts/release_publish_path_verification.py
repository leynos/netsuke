"""Check the inventory-verification job's dependencies, privilege, and guard.

The job sits after both publishers because it verifies what they produced.
That position is only useful if the job's dependencies stay exact: a dependency
on the rehearsal job would let a dry run verify an inventory that was never
published, and an extra write permission would widen the credential the
download steps already need. The reachability oracle is deliberately a separate
computation from the job's own expression, so a guard that silently stops
matching the scenarios is reported rather than agreed with.
"""

import typing as typ

from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_scenarios import (
    GITHUB_SHA,
    VERIFY_NEEDS,
    admits_candidate,
    needs_context,
)
from workflow_loading import workflow_job

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario

VERIFY_JOB = "verify-release-inventory"
#: Draft releases are visible only with write access, so the listing steps
#: need it. Nothing else belongs here: the job reads and writes no repository
#: state beyond that visibility.
VERIFY_PERMISSIONS = {"contents": "write"}


def _declared_names(job: dict[str, object], violations: list[str]) -> list[str] | None:
    """Return the job's dependency names, or report a malformed declaration."""
    names = job_names(job.get("needs", []))
    if names is None:
        violations.append(f"{VERIFY_JOB}.needs: dependencies must be job names")
    return names


def check_verification_needs(job: dict[str, object], violations: list[str]) -> None:
    """Require exactly metadata, the gate, and both publishers.

    The length comparison catches a repeated name, which set equality alone
    would accept and which GitHub treats as an error.
    """
    names = _declared_names(job, violations)
    if names is None:
        return
    if set(names) != VERIFY_NEEDS or len(names) != len(VERIFY_NEEDS):
        violations.append(
            f"{VERIFY_JOB}.needs: expected exactly metadata, package-crate, "
            "release-admission-canaries, publish-crates-io and publish-release"
        )


def check_verification_permissions(
    job: dict[str, object], violations: list[str]
) -> None:
    """Require write-on-contents only, and no deployment environment."""
    if job.get("permissions", MISSING) != VERIFY_PERMISSIONS:
        violations.append(
            f"{VERIFY_JOB}.permissions: expected contents: write and nothing else"
        )
    if "environment" in job:
        violations.append(
            f"{VERIFY_JOB}.environment: verification must not require an "
            "environment's secrets or protection rules"
        )


def _verification_needs_context(
    job: dict[str, object], scenario: Scenario, outputs: dict[str, str]
) -> dict[str, object]:
    """Build the needs context from the job's declared dependencies."""
    return needs_context(job_names(job.get("needs", [])) or [], scenario, outputs)


def _should_verify(scenario: Scenario) -> bool:
    """Compute the independent reachability oracle for inventory verification."""
    return (
        scenario.mode == "publish"
        and not scenario.cancelled
        and admits_candidate(scenario)
        and all(scenario.needs_results.get(name) == "success" for name in VERIFY_NEEDS)
    )


def check_verify_inventory(
    release: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    violations: list[str],
) -> None:
    """Verify the job's shape and its reachability for one scenario."""
    job = workflow_job(release, VERIFY_JOB)
    check_verification_needs(job, violations)
    check_verification_permissions(job, violations)
    contexts: dict[str, object] = {
        "needs": _verification_needs_context(job, scenario, outputs),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
    }
    actual = evaluate_field(
        job.get("if", True),
        FieldEvaluation(
            contexts=contexts,
            violations=violations,
            label=f"{scenario.name}.verify-job.guard",
            job_level=True,
        ),
    )
    expected = _should_verify(scenario)
    runs = actual is not MISSING and bool(actual)
    if runs != expected:
        violations.append(
            f"{scenario.name}.verify-job: resolved to {runs!r}, expected {expected!r}"
        )
