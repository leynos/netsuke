"""Check the isolated write-enabled half of release publication."""

import dataclasses
import typing as typ

from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from workflow_loading import (
    job_steps,
    named_step,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario


@dataclasses.dataclass(frozen=True, slots=True)
class PublicationCheck:
    """Bundle scenario state used to evaluate the publication path."""

    release: dict[str, object]
    scenario: Scenario
    outputs: dict[str, str]
    release_runs: bool
    violations: list[str]


@dataclasses.dataclass(frozen=True, slots=True)
class UploadPlanCheck:
    """Bundle expected plan mode and expression context for an upload step."""

    scenario: Scenario
    contexts: dict[str, object]
    violations: list[str]
    expected_plan: bool
    label: str


def _step_runs(
    step: dict[str, object],
    contexts: dict[str, object],
    violations: list[str],
    label: str,
) -> bool:
    """Evaluate one optional step guard; an absent guard means the step runs."""
    value = evaluate_field(
        step.get("if", True),
        FieldEvaluation(contexts=contexts, violations=violations, label=label),
    )
    return value is not MISSING and bool(value)


def _check_upload_plan(
    steps: list[dict[str, object]],
    check: UploadPlanCheck,
) -> None:
    """Require the release action to match its staging or publication mode."""
    upload = named_step(steps, "Upload artefacts to release")
    upload_with = require_mapping(upload.get("with"), "upload-release-assets.with")
    dry_run = upload_with.get("dry-run", MISSING)
    if dry_run is MISSING:
        check.violations.append(
            f"{check.scenario.name}.{check.label}: dry-run input is missing"
        )
        return
    value = evaluate_field(
        dry_run,
        FieldEvaluation(
            contexts=check.contexts,
            violations=check.violations,
            label=f"{check.scenario.name}.{check.label}",
        ),
    )
    actual_plan = str(value).lower() == "true" if value is not MISSING else False
    if actual_plan != check.expected_plan:
        check.violations.append(
            f"{check.scenario.name}.{check.label}: resolved to {value!r}, "
            f"expected {check.expected_plan!r}"
        )


def _check_publication_step_order(
    steps: list[dict[str, object]], violations: list[str]
) -> None:
    """Keep draft creation before download, hoist, and publication upload."""
    try:
        order = (
            step_index_by_key(steps, "name", "Ensure release exists (draft)"),
            step_index_by_key(steps, "uses", "actions/download-artifact@"),
            step_index_by_key(steps, "run", "hoist_binstall_archives.py"),
            step_index_by_key(steps, "id", "upload_assets"),
        )
    except AssertionError as error:
        violations.append(
            f"publish-release.step-order: staging step is missing: {error}"
        )
        return
    if tuple(sorted(order)) != order:
        violations.append(
            "publish-release.step-order: draft, download, hoist, upload order changed"
        )


def _publication_needs(
    job: dict[str, object],
    check: PublicationCheck,
) -> dict[str, object]:
    """Resolve only the publication job's declared metadata and staging needs."""
    raw_needs = job.get("needs", [])
    declared = [raw_needs] if isinstance(raw_needs, str) else raw_needs
    if not isinstance(declared, list) or any(
        not isinstance(name, str) for name in declared
    ):
        check.violations.append("publish-release.needs: dependencies must be job names")
        return {}
    required = {"metadata", "release"}
    declared_needs = set(declared)
    if declared_needs != required:
        check.violations.append(
            "publish-release.needs: expected metadata and release dependencies"
        )
    needs: dict[str, object] = {}
    if "metadata" in declared_needs:
        needs["metadata"] = {
            "result": check.scenario.needs_results["metadata"],
            "outputs": check.outputs,
        }
    if "release" in declared_needs:
        needs["release"] = {
            "result": "cancelled"
            if check.scenario.cancelled
            else "success"
            if check.release_runs
            else "skipped"
        }
    return needs


def _check_publication_job(
    job: dict[str, object], check: PublicationCheck, contexts: dict[str, object]
) -> bool:
    """Evaluate publication-job reachability and report a mismatched guard."""
    scenario = check.scenario
    actual_job = evaluate_field(
        job.get("if", True),
        FieldEvaluation(
            contexts=contexts,
            violations=check.violations,
            label=f"{scenario.name}.publish-job.guard",
            job_level=True,
        ),
    )
    expected_job = scenario.mode == "publish" and check.release_runs
    publication_runs = actual_job is not MISSING and bool(actual_job)
    if publication_runs != expected_job:
        check.violations.append(
            f"{scenario.name}.publish-job: resolved to {publication_runs!r}, "
            f"expected {expected_job!r}"
        )
    return publication_runs and expected_job


def _check_draft_guard(
    steps: list[dict[str, object]],
    check: PublicationCheck,
    contexts: dict[str, object],
) -> None:
    """Require draft creation to run only in tag-publish scenarios."""
    scenario = check.scenario
    draft = named_step(steps, "Ensure release exists (draft)")
    draft_runs = _step_runs(
        draft, contexts, check.violations, f"{scenario.name}.draft.guard"
    )
    expected_draft = scenario.mode == "publish"
    if draft_runs != expected_draft:
        check.violations.append(
            f"{scenario.name}.draft: resolved to {draft_runs!r}, "
            f"expected {expected_draft!r}"
        )


def _check_publication_steps(
    steps: list[dict[str, object]],
    check: PublicationCheck,
    contexts: dict[str, object],
) -> None:
    """Require publication downloads, hoisting and upload in the correct order."""
    scenario = check.scenario
    for label, index in (
        ("download", step_index_by_key(steps, "uses", "actions/download-artifact@")),
        ("hoist", step_index_by_key(steps, "run", "hoist_binstall_archives.py")),
        ("upload", step_index_by_key(steps, "id", "upload_assets")),
    ):
        if not _step_runs(
            steps[index],
            contexts,
            check.violations,
            f"{scenario.name}.publish-{label}.guard",
        ):
            check.violations.append(
                f"{scenario.name}.publish-{label}: step is guarded off"
            )
    _check_upload_plan(
        steps,
        UploadPlanCheck(
            scenario=scenario,
            contexts=contexts,
            violations=check.violations,
            expected_plan=False,
            label="publish-upload-plan",
        ),
    )
    _check_publication_step_order(steps, check.violations)


def check_publication_path(check: PublicationCheck) -> None:
    """Verify publish-job reachability and the draft and asset upload guards."""
    scenario = check.scenario
    job = workflow_job(check.release, "publish-release")
    contexts: dict[str, object] = {
        "needs": _publication_needs(job, check),
        "status": {"cancelled": scenario.cancelled},
        "github": {"event": {"action": scenario.event_action}},
    }
    publication_runs = _check_publication_job(job, check, contexts)
    steps = job_steps(check.release, "publish-release")
    _check_draft_guard(steps, check, contexts)
    if publication_runs:
        _check_publication_steps(steps, check, contexts)
