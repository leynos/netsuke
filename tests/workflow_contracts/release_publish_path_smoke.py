"""Check the Windows smoke job's reachability for each release mode and event.

The smoke job is skipped on a pull-request rehearsal and runs on a tagged
release, so its guard reads both the release mode and the triggering event.
Evaluating it in every scenario is what stops a guard that quietly runs the
smoke on every dry run, or stops running it on the one dry-run event the
pull-request workflow does not cover.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from workflow_loading import workflow_job

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario

SMOKE_JOB = "windows-native-recipe-smoke"


def check_smoke_job(
    release: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    results: dict[str, str],
    violations: list[str],
) -> None:
    """Check smoke-job reachability for each mode and event.

    Parameters
    ----------
    release
        Parsed release workflow.
    scenario
        The release mode, event and resolved dependency results under test.
    outputs
        The metadata outputs the guard reads.
    results
        Every job's resolved result for the scenario.
    violations
        Collector for the scenario's findings.
    """
    smoke_job = workflow_job(release, SMOKE_JOB)
    metadata_result = results.get("metadata", "skipped")
    context: dict[str, object] = {
        "needs": {
            "metadata": {"result": metadata_result, "outputs": outputs},
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
    should_run = metadata_result == "success" and (
        scenario.mode != "dry-run" or scenario.event_action == "ready_for_review"
    )
    if value is MISSING or bool(value) != should_run:
        violations.append(
            f"{scenario.name}.smoke-job: guard did not resolve to {should_run!r}"
        )


__all__ = ("SMOKE_JOB", "check_smoke_job")
