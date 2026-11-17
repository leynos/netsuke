"""Evaluate the finalization job's reachability for every release scenario.

Un-drafting is irreversible, so `finalize-github-release` is the write job whose
guard matters most. Its exact clause set is held separately by
:mod:`release_write_authority_contract`, but a clause set says nothing about
whether the job can ever *start*: a guard that keeps every required clause and
also names the rehearsal job -- which is skipped on a publishing run -- is
exact and permanently unreachable. This module evaluates the job against each
scenario, so a guard that reaches finalization on a run that should not, or
never reaches it on the run that should, is reported by name.

The reachability oracle is computed from the scenario rather than read off the
guard, so a guard that silently stops matching the model is a finding instead
of an agreed answer.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_context import needs_context
from release_publish_path_publication import step_runs
from release_publish_path_scenarios import (
    FINALIZE_JOB,
    GITHUB_SHA,
    VERIFY_JOB,
    admits_candidate,
)
from workflow_loading import job_steps, named_step, workflow_job

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario

#: The finalization step, named as the workflow spells it.
FINALIZE_STEP = "Finalize the verified release"
#: Every job finalization must wait on. Packaging supplies the identity it
#: reconciles, the verifier the inventory it trusts, the admission gate the
#: authority to publish, and both publishers the release the other two describe.
FINALIZE_NEEDS = {
    "metadata",
    "package-crate",
    "release-admission-canaries",
    "publish-crates-io",
    "publish-release",
    VERIFY_JOB,
}


def _declared_needs(job: dict[str, object], violations: list[str]) -> list[str] | None:
    """Return the job's dependency names, or report a malformed declaration."""
    names = job_names(job.get("needs", []))
    if names is None:
        violations.append(f"{FINALIZE_JOB}.needs: dependencies must be job names")
    return names


def check_finalization_needs(job: dict[str, object], violations: list[str]) -> None:
    """Require exactly the six jobs finalization reconciles a release against."""
    names = _declared_needs(job, violations)
    if names is None:
        return
    if set(names) != FINALIZE_NEEDS or len(names) != len(FINALIZE_NEEDS):
        violations.append(
            f"{FINALIZE_JOB}.needs: expected exactly metadata, package-crate, "
            "release-admission-canaries, publish-crates-io, publish-release and "
            "verify-release-inventory"
        )


def _should_finalize(
    scenario: Scenario,
    outputs: dict[str, str],
    results: dict[str, str],
) -> bool:
    """Compute the independent reachability oracle for finalization."""
    return (
        scenario.mode == "publish"
        and not scenario.cancelled
        and outputs.get("should_publish") == "true"
        and admits_candidate(scenario)
        and all(results.get(name) == "success" for name in FINALIZE_NEEDS)
    )


def _finalize_contexts(
    job: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    results: dict[str, str],
) -> dict[str, object]:
    """Build the guard context from the job's own declared dependencies."""
    return {
        "needs": needs_context(
            job_names(job.get("needs", [])) or [], scenario, outputs, results
        ),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
    }


def check_finalize_release(
    release: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    results: dict[str, str],
    violations: list[str],
) -> None:
    """Verify the job's shape and its reachability for one scenario."""
    job = workflow_job(release, FINALIZE_JOB)
    check_finalization_needs(job, violations)
    contexts = _finalize_contexts(job, scenario, outputs, results)
    actual = evaluate_field(
        job.get("if", True),
        FieldEvaluation(
            contexts=contexts,
            violations=violations,
            label=f"{scenario.name}.finalize-job.guard",
            job_level=True,
        ),
    )
    expected = _should_finalize(scenario, outputs, results)
    runs = actual is not MISSING and bool(actual)
    if runs != expected:
        violations.append(
            f"{scenario.name}.finalize-job: resolved to {runs!r}, expected {expected!r}"
        )
    if runs and expected:
        _check_finalize_step(release, scenario, contexts, violations)


def _check_finalize_step(
    release: dict[str, object],
    scenario: Scenario,
    contexts: dict[str, object],
    violations: list[str],
) -> None:
    """Require the step that un-drafts the release to run when the job does."""
    step = named_step(job_steps(release, FINALIZE_JOB), FINALIZE_STEP)
    if not step_runs(
        step, contexts, violations, f"{scenario.name}.finalize-step.guard"
    ):
        violations.append(
            f"{scenario.name}.finalize-step: the release would stay a draft"
        )


__all__ = (
    "FINALIZE_NEEDS",
    "FINALIZE_STEP",
    "check_finalization_needs",
    "check_finalize_release",
)
