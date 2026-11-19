"""Prove dry runs reach release staging without publishing a release.

``check_release_publish_path`` combines the caller's trigger and input
contract, the staging job's needs and step order, the artifact and download
names, and the publication gate into one list of violations. Each scenario in
``release_publish_path_scenarios`` is then resolved by
``release_publish_path_results`` and checked for the reachability it declares,
so a rehearsal stays reachable after a failed package while no write job
starts. ``release_publish_path_mutations_test.py`` proves every check reports
its labelled regression.
"""

import dataclasses

from action_run_sources import action_sources
from release_admission_gate_contract import (
    check_admission_enforcement,
    check_admission_source_pin,
)
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
from release_publish_path_consumer_smoke import (
    CONSUMER_SMOKE_ACTION_PATH,
    ConsumerSmokeCheck,
    check_consumer_smoke,
)
from release_publish_path_context import ScenarioRun, needs_context
from release_publish_path_finalization import check_finalize_release
from release_publish_path_metadata import metadata_outputs
from release_publish_path_publication import (
    PublicationCheck,
    UploadPlanCheck,
    check_crates_io_publication,
    check_publication_path,
    check_upload_plan,
    step_runs,
)
from release_publish_path_results import resolve_results
from release_publish_path_scenario_table import SCENARIOS
from release_publish_path_scenarios import (
    GITHUB_SHA,
    MODE_OUTPUTS,
    PACKAGE_UPLOAD,
    REQUIRED_DRY_RUN_EVENTS,
    STAGING_FORBIDDEN_NEEDS,
    STAGING_NEEDS,
    Scenario,
)
from release_publish_path_smoke import check_smoke_job
from release_publish_path_verification import check_verify_inventory
from release_write_authority_contract import check_publication_gate
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
    outputs_by_mode: dict[str, dict[str, str]]
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


def _stage_needs(
    release_job: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    results: dict[str, str],
) -> dict[str, object]:
    """Build the declared needs context for one staging or publishing job."""
    return needs_context(
        job_names(release_job.get("needs", [])) or [],
        ScenarioRun(scenario, outputs, results),
    )


def _check_caller(
    release: dict[str, object], caller: dict[str, object], violations: list[str]
) -> None:
    """Require the dry-run caller and reusable release defaults to stay safe."""
    _check_caller_events(caller, violations)

    caller_job = workflow_job(caller, "release")
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
        scenario.event_action for scenario in SCENARIOS if scenario.mode == "dry-run"
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
    """Require staging to depend on the builds, and on nothing that can refuse.

    A dependency on packaging or admission would make a dry run unreachable
    whenever either fails, which is the read-only lane's whole purpose.
    """
    needs = job_names(workflow_job(release, "release").get("needs", [])) or []
    declared_needs = set(needs)
    missing_needs = STAGING_NEEDS - declared_needs
    if missing_needs:
        violations.append(f"release.needs: missing {sorted(missing_needs)!r}")
    gated_needs = STAGING_FORBIDDEN_NEEDS & declared_needs
    if gated_needs:
        violations.append(f"release.needs: must not depend on {sorted(gated_needs)!r}")


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
    """Evaluate dependency results, package uploads, and release staging.

    The scenario's declared outcomes are propagated through the graph first, so
    every guard below is evaluated against a run the workflow could produce
    rather than against independently supplied results.
    """
    scenario = check.scenario
    outputs = check.outputs_by_mode[scenario.mode]
    run = resolve_results(check.release, scenario, outputs, check.violations)
    check_smoke_job(check.release, run, check.violations)
    check_admission_enforcement(check.release, scenario, outputs, check.violations)
    check_build_uploads(
        check.release,
        check.build,
        PackageUploadScenario(scenario.name, outputs, PACKAGE_UPLOAD[scenario.mode]),
        check.violations,
    )
    release_job = workflow_job(check.release, "release")
    contexts: dict[str, object] = {
        "needs": _stage_needs(release_job, scenario, outputs, run.results),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
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
    check_publication_path(publication_check, run.results)
    publisher_job = workflow_job(check.release, "publish-crates-io")
    check_finalize_release(check.release, run, check.violations)
    publisher_contexts: dict[str, object] = {
        "needs": _stage_needs(publisher_job, scenario, outputs, run.results),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
    }
    check_crates_io_publication(
        publisher_job, publication_check, publisher_contexts, run.results
    )
    check_verify_inventory(check.release, run, check.violations)
    check_consumer_smoke(
        ConsumerSmokeCheck(
            release=check.release,
            action=check.smoke_action,
            scenario=scenario,
            outputs=outputs,
            results=run.results,
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
    check_publication_gate(release, caller, violations)
    check_admission_source_pin(release, violations)
    outputs_by_mode = {
        mode: metadata_outputs(release, mode, violations) for mode in MODE_OUTPUTS
    }
    check_artifact_names(
        release,
        build,
        outputs_by_mode["dry-run"].get("repo_name", REPO_ROOT.name),
        violations,
    )
    check_download_pattern(
        release,
        outputs_by_mode["dry-run"].get("repo_name", REPO_ROOT.name),
        violations,
    )
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
                outputs_by_mode=outputs_by_mode,
                scenario=scenario,
                violations=violations,
                smoke_action=smoke_action,
            )
        )
    return violations
