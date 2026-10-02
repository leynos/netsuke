"""Prove dry runs reach release staging without publishing a release."""

import dataclasses

from release_publish_path_artifacts import (
    MISSING,
    FieldEvaluation,
    PackageUploadScenario,
    check_artifact_names,
    check_build_uploads,
    check_download_pattern,
    evaluate_field,
)
from release_publish_path_publication import (
    PublicationCheck,
    UploadPlanCheck,
    _check_upload_plan,
    _step_runs,
    check_publication_path,
)
from release_publish_path_scenarios import (
    _MODE_OUTPUTS,
    _PACKAGE_UPLOAD,
    REQUIRED_DRY_RUN_EVENTS,
    REQUIRED_NEEDS,
    SCENARIOS,
    Scenario,
)
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_list,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

CALLER_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-dry-run.yml"


@dataclasses.dataclass(frozen=True, slots=True)
class ScenarioCheck:
    """Bundle the workflows and scenario state checked as one release run."""

    release: dict[str, object]
    build: dict[str, object]
    outputs_by_mode: dict[str, dict[str, str]]
    scenario: Scenario
    violations: list[str]


def _workflow_call_inputs(workflow: dict[str, object]) -> dict[str, object]:
    """Return a reusable workflow's declared inputs."""
    triggers = require_mapping(
        workflow.get("on", workflow.get(True)), "workflow triggers"
    )
    call = require_mapping(triggers.get("workflow_call"), "workflow_call")
    return require_mapping(call.get("inputs"), "workflow_call.inputs")


def _metadata_outputs(
    release: dict[str, object], mode: str, violations: list[str]
) -> dict[str, str]:
    """Resolve the release-mode outputs declared by the metadata job."""
    declarations = require_mapping(
        workflow_job(release, "metadata").get("outputs"), "metadata.outputs"
    )
    steps = {
        "release_modes": {"outputs": _MODE_OUTPUTS[mode]},
        "repo_name": {"outputs": {"value": REPO_ROOT.name}},
    }
    contexts: dict[str, object] = {"steps": steps}
    outputs: dict[str, str] = {}
    expected = {
        "should_publish": _MODE_OUTPUTS[mode]["should-publish"],
        "dry_run": _MODE_OUTPUTS[mode]["dry-run"],
        "should_upload_workflow_artifacts": _MODE_OUTPUTS[mode][
            "should-upload-workflow-artifacts"
        ],
        "should_upload_package_artifacts": str(_PACKAGE_UPLOAD[mode]).lower(),
        "repo_name": REPO_ROOT.name,
    }
    for name, expected_value in expected.items():
        expression = declarations.get(name, MISSING)
        label = f"metadata.{name}"
        if expression is MISSING:
            violations.append(f"{label}: output is missing")
            continue
        value = evaluate_field(
            expression,
            FieldEvaluation(contexts=contexts, violations=violations, label=label),
        )
        if value is MISSING:
            continue
        rendered = str(value).lower() if isinstance(value, bool) else value
        if not isinstance(rendered, str):
            violations.append(f"{label}: output did not resolve to a string or boolean")
            continue
        outputs[name] = rendered
        if rendered != expected_value:
            violations.append(
                f"{label}: resolved to {rendered!r}, expected {expected_value!r}"
            )
    return outputs


def _needs_context(
    release_job: dict[str, object], scenario: Scenario, outputs: dict[str, str]
) -> dict[str, object]:
    """Build only the needs context the release job actually declares."""
    declared = release_job.get("needs", [])
    names = [declared] if isinstance(declared, str) else declared
    if not isinstance(names, list):
        return {}
    needs: dict[str, object] = {}
    for name in names:
        if not isinstance(name, str) or name not in scenario.needs_results:
            continue
        dependency: dict[str, object] = {"result": scenario.needs_results[name]}
        if name == "metadata":
            dependency["outputs"] = outputs
        needs[name] = dependency
    return needs


def _check_smoke_job(
    release: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    violations: list[str],
) -> None:
    """Check smoke-job reachability for each mode and event."""
    smoke_job = workflow_job(release, "windows-native-recipe-smoke")
    context: dict[str, object] = {
        "needs": {
            "metadata": {
                "result": scenario.needs_results["metadata"],
                "outputs": outputs,
            }
        },
        # A cancelled release scenario cancels after every dependency passed.
        "status": {"cancelled": False},
        "github": {"event": {"action": scenario.event_action}},
    }
    value = evaluate_field(
        smoke_job.get("if", True),
        FieldEvaluation(
            contexts=context,
            violations=violations,
            label=f"{scenario.name}.smoke-job.guard",
            job_level=True,
        ),
    )
    should_run = (
        scenario.mode != "dry-run" or scenario.event_action == "ready_for_review"
    )
    if value is MISSING or bool(value) != should_run:
        violations.append(
            f"{scenario.name}.smoke-job: guard did not resolve to {should_run!r}"
        )


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
        if not _step_runs(
            steps[index], contexts, violations, f"{scenario.name}.{label}.guard"
        ):
            violations.append(f"{scenario.name}.{label}: staging step is guarded off")

    plan_index = step_index_by_key(steps, "id", "upload_assets")
    plan_runs = _step_runs(
        steps[plan_index], contexts, violations, f"{scenario.name}.upload.guard"
    )
    expected_plan_step = True
    if plan_runs != expected_plan_step:
        violations.append(
            f"{scenario.name}.upload: staging plan step resolved to {plan_runs!r}, "
            f"expected {expected_plan_step!r}"
        )
    _check_upload_plan(
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
    needs = workflow_job(release, "release").get("needs", [])
    declared_needs = set(needs if isinstance(needs, list) else [needs])
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
    outputs = check.outputs_by_mode[scenario.mode]
    _check_smoke_job(check.release, scenario, outputs, check.violations)
    check_build_uploads(
        check.release,
        check.build,
        PackageUploadScenario(scenario.name, outputs, _PACKAGE_UPLOAD[scenario.mode]),
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
    check_publication_path(
        PublicationCheck(
            release=check.release,
            scenario=scenario,
            outputs=outputs,
            violations=check.violations,
        )
    )


def check_release_publish_path(
    release: dict[str, object],
    caller: dict[str, object],
    build: dict[str, object],
) -> list[str]:
    """Return violations in the dry-run and tag-publish reachability contract.

    The checker is pure with respect to its inputs: it evaluates the parsed
    workflows against a finite scenario table and performs no filesystem or
    network access.

    Returns
    -------
    list[str]
        One entry for every missing or incorrectly wired release-path invariant.
    """
    violations: list[str] = []
    _check_caller(release, caller, violations)
    outputs_by_mode = {
        mode: _metadata_outputs(release, mode, violations) for mode in _MODE_OUTPUTS
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
    for scenario in SCENARIOS:
        _check_scenario(
            ScenarioCheck(
                release=release,
                build=build,
                outputs_by_mode=outputs_by_mode,
                scenario=scenario,
                violations=violations,
            )
        )
    return violations


def test_release_dry_run_reaches_the_staging_path() -> None:
    """Run the release-path contract over every dry-run and publish scenario."""
    violations = check_release_publish_path(
        load_workflow(RELEASE_WORKFLOW_PATH),
        load_workflow(CALLER_WORKFLOW_PATH),
        load_workflow(PACKAGE_WORKFLOW_PATH),
    )
    assert not violations, "release publish-path contract violations:\n" + "\n".join(
        violations
    )
