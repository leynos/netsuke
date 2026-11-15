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

import dataclasses
import typing as typ

from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_consumer_shape import (
    CANDIDATE_REF,
    CHECKOUT_PREFIX,
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    CONSUMER_SMOKE_NEEDS,
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

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario


@dataclasses.dataclass(frozen=True, slots=True)
class ConsumerSmokeCheck:
    """Bundle the smoke documents and scenario state checked as one run.

    Attributes
    ----------
    release
        Parsed release workflow, mutated in place by a caller's deep copy.
    action
        Parsed consumer-smoke manifest, or ``None`` when it is missing.
    scenario
        The release mode, event, and dependency outcomes being resolved.
    outputs
        The metadata outputs the guard expression reads.
    violations
        The violations collected so far, appended to in place.
    """

    release: dict[str, object]
    action: dict[str, object] | None
    scenario: Scenario
    outputs: dict[str, str]
    violations: list[str]


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


def check_consumer_smoke(check: ConsumerSmokeCheck) -> None:
    """Verify the job's shape and its reachability for one scenario."""
    job = smoke_job(check.release, check.violations)
    check_smoke_needs(job, check.violations)
    check_smoke_privileges(job, check.violations)
    check_smoke_checkout(job, check.violations)
    check_smoke_artifacts(job, check.violations)
    check_smoke_action_inputs(job, check.violations)
    check_smoke_action_declared(check.action, check.violations)
    check_smoke_compilation(job, check.action, check.violations)
    contexts: dict[str, object] = {
        "needs": _needs_context(job, check.scenario, check.outputs),
        "status": {"cancelled": check.scenario.cancelled},
        "github": {"event": {"action": check.scenario.event_action}},
    }
    actual = evaluate_field(
        job.get("if", True),
        FieldEvaluation(
            contexts=contexts,
            violations=check.violations,
            label=f"{check.scenario.name}.consumer-smoke-job.guard",
            job_level=True,
        ),
    )
    expected = _should_smoke(check.scenario)
    runs = actual is not MISSING and bool(actual)
    if runs != expected:
        check.violations.append(
            f"{check.scenario.name}.consumer-smoke-job: resolved to {runs!r}, "
            f"expected {expected!r}"
        )


__all__ = [
    "CANDIDATE_REF",
    "CHECKOUT_PREFIX",
    "CONSUMER_SMOKE_ACTION_PATH",
    "CONSUMER_SMOKE_JOB",
    "CONSUMER_SMOKE_NEEDS",
    "CONSUMER_SMOKE_PERMISSIONS",
    "LOCAL_SMOKE_ACTION",
    "PACKAGE_RECORD_ARTIFACT",
    "SMOKE_ACTION_INPUTS",
    "SMOKE_ACTION_TARGET",
    "SMOKE_ARTIFACTS",
    "ConsumerSmokeCheck",
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
