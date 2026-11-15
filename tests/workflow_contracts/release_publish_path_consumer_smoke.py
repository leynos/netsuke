"""Check the consumer-smoke job's shape, privilege, and reachability.

The job's shape and privilege are asserted in
``release_publish_path_consumer_shape``; this module composes those checks with
the reachability oracle, so one call answers both "is the smoke shaped the way
it must be" and "does its guard run it exactly when the release is publishable".
The two questions are separate on purpose: a shape check cannot notice that a
guard stopped matching the scenarios, and a reachability check cannot notice
that the job it agrees with holds a credential.

The reachability oracle is computed independently of the job's own expression,
so a guard that stops matching the scenarios is reported rather than agreed
with. Everything the shape module defines is re-exported here, because this is
the module the release-path contract and the mutation suite import.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_consumer_shape import (
    CANDIDATE_REF,
    CHECKOUT_PREFIX,
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    CONSUMER_SMOKE_PERMISSIONS,
    LOCAL_SMOKE_ACTION,
    PACKAGE_RECORD_ARTIFACT,
    SMOKE_ACTION_INPUTS,
    SMOKE_ACTION_TARGET,
    SMOKE_ARTIFACTS,
    action_steps,
    check_smoke_action_declared,
    check_smoke_action_inputs,
    check_smoke_artifacts,
    check_smoke_checkout,
    check_smoke_compilation,
    check_smoke_needs,
    check_smoke_privileges,
    smoke_job,
    steps_of,
)
from release_publish_path_scenarios import CONSUMER_SMOKE_NEEDS

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario


def _needs_context(
    job: dict[str, object], scenario: Scenario, outputs: dict[str, str]
) -> dict[str, object]:
    """Build the needs context from the job's declared dependencies.

    Read from the job rather than from ``CONSUMER_SMOKE_NEEDS`` so a mutated
    dependency list is evaluated as written and reports its own result, instead
    of being silently replaced by the oracle's expectation.

    Returns
    -------
    dict[str, object]
        A needs entry per declared dependency, carrying its scenario result.
    """
    needs: dict[str, object] = {}
    for name in job_names(job.get("needs", [])) or []:
        dependency: dict[str, object] = {
            "result": scenario.needs_results.get(name, "skipped")
        }
        if name == "metadata":
            dependency["outputs"] = outputs
        needs[name] = dependency
    return needs


def _should_smoke(scenario: Scenario) -> bool:
    """Compute the independent reachability oracle for the consumer smoke."""
    return (
        scenario.mode == "publish"
        and not scenario.cancelled
        and all(
            scenario.needs_results.get(name) == "success"
            for name in CONSUMER_SMOKE_NEEDS
        )
    )


def check_consumer_smoke(
    release: dict[str, object],
    action: dict[str, object] | None,
    scenario: Scenario,
    outputs: dict[str, str],
    violations: list[str],
) -> None:
    """Verify the job's shape and its reachability for one scenario."""
    job = smoke_job(release, violations)
    check_smoke_needs(job, CONSUMER_SMOKE_NEEDS, violations)
    check_smoke_privileges(job, violations)
    check_smoke_checkout(job, violations)
    check_smoke_artifacts(job, violations)
    check_smoke_action_inputs(job, violations)
    check_smoke_action_declared(action, violations)
    check_smoke_compilation(job, action, violations)
    contexts: dict[str, object] = {
        "needs": _needs_context(job, scenario, outputs),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}},
    }
    actual = evaluate_field(
        job.get("if", True),
        FieldEvaluation(
            contexts=contexts,
            violations=violations,
            label=f"{scenario.name}.consumer-smoke-job.guard",
            job_level=True,
        ),
    )
    expected = _should_smoke(scenario)
    runs = actual is not MISSING and bool(actual)
    if runs != expected:
        violations.append(
            f"{scenario.name}.consumer-smoke-job: resolved to {runs!r}, "
            f"expected {expected!r}"
        )


__all__ = [
    "CANDIDATE_REF",
    "CHECKOUT_PREFIX",
    "CONSUMER_SMOKE_ACTION_PATH",
    "CONSUMER_SMOKE_JOB",
    "CONSUMER_SMOKE_PERMISSIONS",
    "LOCAL_SMOKE_ACTION",
    "PACKAGE_RECORD_ARTIFACT",
    "SMOKE_ACTION_INPUTS",
    "SMOKE_ACTION_TARGET",
    "SMOKE_ARTIFACTS",
    "action_steps",
    "check_consumer_smoke",
    "check_smoke_action_declared",
    "check_smoke_action_inputs",
    "check_smoke_artifacts",
    "check_smoke_checkout",
    "check_smoke_compilation",
    "check_smoke_needs",
    "check_smoke_privileges",
    "smoke_job",
    "steps_of",
]
