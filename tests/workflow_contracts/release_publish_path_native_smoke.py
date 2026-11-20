"""Check the native-recipe smoke job's guard on the release workflow.

`windows-native-recipe-smoke` runs the Windows native recipe against the build a
dry run produced as well as against a publishing run's build, and it is the one
job whose guard has to tell those two modes apart without widening either. The
check lives here rather than in the release-path orchestration so that module
stays inside the module size limit, and so the guard can be exercised on its own
without loading every other release-path contract.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from workflow_loading import workflow_job

if typ.TYPE_CHECKING:
    from release_publish_path_scenario_model import Scenario


def check_native_smoke_job(
    release: dict[str, object],
    scenario: Scenario,
    outputs: dict[str, str],
    violations: list[str],
) -> None:
    """Check smoke-job reachability for each mode and event.

    The job depends on `metadata` alone, so its guard is resolved against that
    one result rather than a full needs context. A cancelled release scenario
    cancels after every dependency passed, which is why the status context here
    never reports a cancellation.
    """
    smoke_job = workflow_job(release, "windows-native-recipe-smoke")
    context: dict[str, object] = {
        "needs": {
            "metadata": {
                "result": scenario.effective_results["metadata"],
                "outputs": outputs,
            }
        },
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
    metadata_succeeded = scenario.effective_results["metadata"] == "success"
    # The guard reads the decision's effective output, not the scenario's
    # nominal mode: a publish-shaped invocation whose authorization still
    # leaves the run in dry-run mode must skip the smoke exactly as a plain
    # dry run does.
    should_run = metadata_succeeded and (
        outputs.get("dry_run") != "true" or scenario.event_action == "ready_for_review"
    )
    if value is MISSING or bool(value) != should_run:
        violations.append(
            f"{scenario.name}.smoke-job: guard did not resolve to {should_run!r}"
        )
