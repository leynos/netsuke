"""Aggregate the release publish-path contract checks.

This module composes the checks that decide whether a release run may reach a
write-enabled job: the authorization wiring, the metadata-output resolver, and
the publication, verification and consumer-smoke paths. The authorization
decision itself comes from ``scripts/authorize_release_writes.py`` through
``release_publish_path_scenario_model``, so the contract checks the same rule a
live run applies. The scenario table lives in
``release_publish_path_scenarios``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses

from action_run_sources import action_sources
from release_publish_crates_io_contracts import (
    ACTION_PATH as CRATES_IO_ACTION_PATH,
)
from release_publish_crates_io_contracts import (
    check_crates_io_action_contract,
)
from release_publish_needs import job_names
from release_publish_path_artifacts import (
    MISSING,
    FieldEvaluation,
    PackageUploadScenario,
    check_artifact_names,
    check_build_uploads,
    check_download_pattern,
    evaluate_field,
)
from release_publish_path_authorization import check_authorization_wiring
from release_publish_path_consumer_smoke import (
    CONSUMER_SMOKE_ACTION_PATH,
    ConsumerSmokeCheck,
    check_consumer_smoke,
)
from release_publish_path_metadata_outputs import (
    metadata_outputs,
    resolved_repo_name,
)
from release_publish_path_native_smoke import check_native_smoke_job
from release_publish_path_publication import (
    PublicationCheck,
    UploadPlanCheck,
    check_crates_io_publication,
    check_publication_path,
    check_upload_plan,
    step_runs,
)
from release_publish_path_scenario_model import (
    REQUIRED_DRY_RUN_EVENTS,
    REQUIRED_NEEDS,
    Scenario,
)
from release_publish_path_scenarios import SCENARIOS
from release_publish_path_verification import check_verify_inventory
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    require_list,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

CALLER_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-dry-run.yml"


@dataclasses.dataclass(frozen=True, slots=True)
class ActionManifests:
    """Name the composite manifests a caller supplies instead of the repository's.

    Attributes
    ----------
    publisher
        Parsed publisher action manifest, or ``None`` to read the repository's.
    consumer_smoke
        Parsed consumer-smoke manifest, or ``None`` to read the repository's.
    """

    publisher: dict[str, object] | None = None
    consumer_smoke: dict[str, object] | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class ScenarioCheck:
    """Bundle the workflows and scenario state checked as one release run."""

    release: dict[str, object]
    build: dict[str, object]
    outputs: dict[str, str]
    scenario: Scenario
    violations: list[str]
    smoke_action: dict[str, object] | None = None


def _workflow_call_inputs(workflow: dict[str, object]) -> dict[str, object]:
    """Return a reusable workflow's declared inputs."""
    triggers = require_mapping(
        workflow.get("on", workflow.get(True)), "workflow triggers"
    )
    call = require_mapping(triggers.get("workflow_call"), "workflow_call")
    return require_mapping(call.get("inputs"), "workflow_call.inputs")


def _needs_context(
    release_job: dict[str, object], scenario: Scenario, outputs: dict[str, str]
) -> dict[str, object]:
    """Build declared needs, defaulting unknown prerequisites to skipped."""
    needs: dict[str, object] = {}
    for name in job_names(release_job.get("needs", [])) or []:
        dependency: dict[str, object] = {
            "result": scenario.needs_results.get(name, "skipped")
        }
        if name == "metadata":
            dependency["outputs"] = outputs
        needs[name] = dependency
    return needs


def _check_caller(
    release: dict[str, object], caller: dict[str, object], violations: list[str]
) -> None:
    """Require the dry-run caller and reusable release defaults to stay safe."""
    _check_caller_events(caller, violations)

    caller_job = workflow_job(caller, "release")
    caller_inputs = require_mapping(caller_job.get("with"), "caller release.with")
    if caller_inputs.get("dry-run") is not True:
        violations.append("caller.dry-run: release-dry-run must pass true")
    if "publish" in caller_inputs:
        violations.append("caller.publish: dry-run caller must omit publish")
    publish = _workflow_call_inputs(release).get("publish")
    publish_input = require_mapping(publish, "release publish input")
    if publish_input.get("default") is not False:
        violations.append("release.publish-default: publish must default to false")
    if caller_job.get("uses") != "./.github/workflows/release.yml":
        violations.append("caller.release-workflow: caller must use release.yml")


def _check_caller_events(caller: dict[str, object], violations: list[str]) -> None:
    """Require caller pull-request triggers to cover every dry-run scenario."""
    triggers = require_mapping(caller.get("on", caller.get(True)), "caller triggers")
    pull_request = require_mapping(triggers.get("pull_request"), "caller pull_request")
    raw_types = require_list(pull_request.get("types"), "caller pull_request.types")
    event_types = {str(event) for event in raw_types}
    scenario_events = {
        scenario.event_action
        for scenario in SCENARIOS
        if scenario.arrival.event_name == "pull_request"
    }
    expected_events = REQUIRED_DRY_RUN_EVENTS
    if event_types != expected_events or scenario_events != expected_events:
        violations.append(
            "caller.pull-request-types: dry-run triggers and scenarios must cover "
            f"{sorted(REQUIRED_DRY_RUN_EVENTS)!r}"
        )


def _check_release_staging(
    release: dict[str, object],
    scenario: Scenario,
    contexts: dict[str, object],
    violations: list[str],
) -> None:
    """Check artifact download, hoist, and plan reachability for staging."""
    steps = job_steps(release, "release")
    staging_steps = (
        ("download", step_index_by_key(steps, "uses", "actions/download-artifact@")),
        ("hoist", step_index_by_key(steps, "run", "hoist_binstall_archives.py")),
    )
    for label, index in staging_steps:
        if not step_runs(
            steps[index], contexts, violations, f"{scenario.name}.{label}.guard"
        ):
            violations.append(f"{scenario.name}.{label}: staging step is guarded off")

    plan_index = step_index_by_key(steps, "id", "upload_assets")
    plan_runs = step_runs(
        steps[plan_index], contexts, violations, f"{scenario.name}.upload.guard"
    )
    expected_plan_step = True
    if plan_runs != expected_plan_step:
        violations.append(
            f"{scenario.name}.upload: staging plan step resolved to {plan_runs!r}, "
            f"expected {expected_plan_step!r}"
        )
    check_upload_plan(
        steps,
        UploadPlanCheck(
            scenario=scenario,
            contexts=contexts,
            violations=violations,
            expected_plan=True,
            label="upload-plan",
        ),
    )


def _check_release_needs(release: dict[str, object], violations: list[str]) -> None:
    """Require release staging to depend on every build and Windows smoke."""
    needs = job_names(workflow_job(release, "release").get("needs", [])) or []
    declared_needs = set(needs)
    missing_needs = REQUIRED_NEEDS - declared_needs
    if missing_needs:
        violations.append(f"release.needs: missing {sorted(missing_needs)!r}")


def _check_release_step_order(
    release: dict[str, object], violations: list[str]
) -> None:
    """Keep download, hoist, and plan steps in staging order."""
    steps = job_steps(release, "release")
    try:
        order = (
            step_index_by_key(steps, "uses", "actions/download-artifact@"),
            step_index_by_key(steps, "run", "hoist_binstall_archives.py"),
            step_index_by_key(steps, "id", "upload_assets"),
        )
    except AssertionError as error:
        violations.append(f"release.step-order: staging step is missing: {error}")
        return
    if tuple(sorted(order)) != order:
        violations.append("release.step-order: download, hoist, plan order changed")


def _check_scenario(check: ScenarioCheck) -> None:
    """Evaluate dependency results, package uploads, and release staging."""
    scenario = check.scenario
    outputs = check.outputs
    check_native_smoke_job(check.release, scenario, outputs, check.violations)
    check_build_uploads(
        check.release,
        check.build,
        PackageUploadScenario(
            scenario.name,
            outputs,
            outputs.get("should_upload_package_artifacts") == "true",
        ),
        check.violations,
    )
    release_job = workflow_job(check.release, "release")
    contexts: dict[str, object] = {
        "needs": _needs_context(release_job, scenario, outputs),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}},
    }
    actual_release = evaluate_field(
        release_job.get("if", True),
        FieldEvaluation(
            contexts=contexts,
            violations=check.violations,
            label=f"{scenario.name}.release-job.guard",
            job_level=True,
        ),
    )
    release_runs = actual_release is not MISSING and bool(actual_release)
    if release_runs != scenario.release_runs:
        check.violations.append(
            f"{scenario.name}.release-job: resolved to {release_runs!r}, "
            f"expected {scenario.release_runs!r}"
        )
    if release_runs and scenario.release_runs:
        _check_release_staging(check.release, scenario, contexts, check.violations)
    publication_check = PublicationCheck(
        release=check.release,
        scenario=scenario,
        outputs=outputs,
        violations=check.violations,
    )
    check_publication_path(publication_check)
    publisher_job = workflow_job(check.release, "publish-crates-io")
    publisher_contexts: dict[str, object] = {
        "needs": _needs_context(publisher_job, scenario, outputs),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}},
    }
    check_crates_io_publication(publisher_job, publication_check, publisher_contexts)
    check_verify_inventory(check.release, scenario, outputs, check.violations)
    check_consumer_smoke(
        ConsumerSmokeCheck(
            release=check.release,
            action=check.smoke_action,
            scenario=scenario,
            outputs=outputs,
            violations=check.violations,
        )
    )


def _check_publisher_action(
    release: dict[str, object],
    publisher_action: dict[str, object] | None,
    violations: list[str],
) -> None:
    """Load an omitted publisher manifest and check the action's wiring."""
    action = (
        publisher_action
        if publisher_action is not None
        else action_sources().get(CRATES_IO_ACTION_PATH)
    )
    if action is None:
        violations.append("publish-crates-io.action: composite manifest is missing")
    else:
        check_crates_io_action_contract(release, action, violations)


def check_release_publish_path(
    release: dict[str, object],
    caller: dict[str, object],
    build: dict[str, object],
    manifests: ActionManifests | None = None,
) -> list[str]:
    """Return violations in the dry-run and tag-publish reachability contract.

    Parameters
    ----------
    release, caller, build : dict[str, object]
        Parsed workflow documents for the release, caller, and build workflows.
    manifests : ActionManifests | None, optional
        Composite action manifests to check instead of the repository's. A
        caller supplies them to check mutated copies.

    Returns
    -------
    list[str]
        Missing or incorrectly wired release-path invariants.
    """
    supplied = manifests if manifests is not None else ActionManifests()
    violations: list[str] = []
    _check_caller(release, caller, violations)
    check_authorization_wiring(release, violations)
    repo_name = resolved_repo_name(release, violations)
    check_artifact_names(release, build, repo_name, violations)
    check_download_pattern(release, repo_name, violations)
    _check_release_needs(release, violations)
    _check_release_step_order(release, violations)
    _check_publisher_action(release, supplied.publisher, violations)
    smoke_action = (
        supplied.consumer_smoke
        if supplied.consumer_smoke is not None
        else action_sources().get(CONSUMER_SMOKE_ACTION_PATH)
    )
    for scenario in SCENARIOS:
        _check_scenario(
            ScenarioCheck(
                release=release,
                build=build,
                outputs=metadata_outputs(release, scenario, violations),
                scenario=scenario,
                violations=violations,
                smoke_action=smoke_action,
            )
        )
    return violations
