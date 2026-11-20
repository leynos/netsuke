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

from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_consumer_artifacts import check_smoke_artifacts
from release_publish_path_consumer_commands import (
    check_smoke_action_commands,
    check_smoke_compilation,
)
from release_publish_path_consumer_credentials import (
    check_smoke_action_environment,
    check_smoke_privileges,
)
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
    check_smoke_checkout,
    check_smoke_failure_visibility,
    check_smoke_needs,
    smoke_job,
    steps_of,
)
from release_publish_path_context import job_contexts
from release_publish_path_scenario_model import Scenario, needs_results


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


def _should_smoke(scenario: Scenario) -> bool:
    """Compute the independent reachability oracle for the consumer smoke.

    The oracle reads the authority the policy granted, not the resolver's
    nominal mode: a publish-shaped invocation that the policy narrows to a dry
    run stages a draft release the smoke cannot download without a token.

    Returns
    -------
    bool
        Whether the smoke must run for this scenario.
    """
    return (
        scenario.authorization.should_publish
        and not scenario.cancelled
        and all(
            scenario.effective_results.get(name) == "success"
            for name in CONSUMER_SMOKE_NEEDS
        )
    )


#: The release modes whose metadata outputs set the smoke's guard, each paired
#: with whether a run that reports them must smoke. Only the mode differs
#: between the two, so a guard that stopped reading it agrees with one of them.
PUBLICATION_GUARD_CASES = (
    ("publish", True),
    ("neither", False),
)


def _static_mode_outputs(mode: str) -> dict[str, str]:
    """Build the metadata outputs a correctly wired run reports for ``mode``."""
    scenario = Scenario(
        name=f"static-{mode}",
        mode=mode,
        event_action="",
        needs_results=needs_results(),
        cancelled=False,
        release_runs=False,
    )
    return scenario.outputs


def check_smoke_publication_guard(
    release: dict[str, object], violations: list[str]
) -> None:
    """Require the guard to admit a publishing run and refuse a rehearsal.

    Every dependency is reported successful and the run is not cancelled, so no
    clause except the mode predicate can decide the result. This is the one
    property the scenario table cannot reach: a run that publishes nothing also
    skips ``finalize-github-release``, whose success the guard already requires,
    so the graph makes the guard false whether or not it reads the mode. Without
    this check the predicate could be deleted and every scenario would still
    agree with the oracle.

    The check runs once rather than per scenario, because it compares two modes
    that no single scenario reports.
    """
    jobs = release.get("jobs")
    job = jobs.get(CONSUMER_SMOKE_JOB) if isinstance(jobs, dict) else None
    if not isinstance(job, dict):
        # A workflow with no smoke job is refused once per scenario by
        # ``check_consumer_smoke``; there is no guard here to judge, and
        # reporting it again would name this check rather than that absence.
        return
    for mode, expected in PUBLICATION_GUARD_CASES:
        outputs = _static_mode_outputs(mode)
        scenario = Scenario(
            name=f"static-{mode}",
            mode=mode,
            event_action="",
            needs_results=needs_results(),
            cancelled=False,
            release_runs=False,
        )
        actual = evaluate_field(
            job.get("if", True),
            FieldEvaluation(
                contexts=job_contexts(scenario, outputs, job),
                violations=violations,
                label=f"{CONSUMER_SMOKE_JOB}.guard",
                job_level=True,
            ),
        )
        if actual is MISSING:
            return
        runs = bool(actual)
        if runs != expected:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.guard: with should_publish="
                f"{outputs.get('should_publish')!r} the guard must resolve to "
                f"{expected!r}, got {runs!r}"
            )


def check_consumer_smoke(check: ConsumerSmokeCheck) -> None:
    """Verify the job's shape and its reachability for one scenario."""
    job = smoke_job(check.release, check.violations)
    check_smoke_needs(job, check.violations)
    check_smoke_privileges(check.release, job, check.violations)
    check_smoke_checkout(job, check.violations)
    check_smoke_artifacts(job, check.violations)
    check_smoke_action_inputs(job, check.violations)
    check_smoke_action_declared(check.action, check.violations)
    check_smoke_action_environment(check.action, check.violations)
    check_smoke_action_commands(check.action, check.violations)
    check_smoke_compilation(job, check.action, check.violations)
    check_smoke_failure_visibility(job, check.action, check.violations)
    contexts = job_contexts(check.scenario, check.outputs, job)
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
    "check_smoke_action_commands",
    "check_smoke_action_declared",
    "check_smoke_action_environment",
    "check_smoke_action_inputs",
    "check_smoke_artifacts",
    "check_smoke_checkout",
    "check_smoke_compilation",
    "check_smoke_failure_visibility",
    "check_smoke_needs",
    "check_smoke_privileges",
    "check_smoke_publication_guard",
    "smoke_job",
    "steps_of",
]
