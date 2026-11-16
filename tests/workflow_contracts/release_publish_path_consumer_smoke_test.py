"""Hold the consumer smoke's telemetry to the release observability contract.

A smoke refusal is diagnosed from which phase refused, so the job must report
the two downloads, the two digest checks, and the executable probe separately,
with bounded outcomes and timings, and retain the record past the runner. The
action is the only place that sees those boundaries, so the job's summary has
to read them from the action's outputs rather than guess — and the action has
to report exactly the phases the catalogue names, or a phase the smoke never
runs would be summarised as skipped on every run.

Run via ``make test-workflow-contracts``.
"""

import pytest
from action_run_sources import action_sources
from release_consumer_payload import SMOKE_PHASES
from release_operation_catalog import OPERATION_PHASES
from release_operation_observability_support import (
    assert_retained_telemetry_policy,
    assert_summary_environment,
    assert_summary_phase_timings,
)
from release_publish_path_consumer_shape import command_lines
from release_publish_path_consumer_smoke import (
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    SMOKE_ACTION_TARGET,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
)

#: The step that calls the local action, and therefore the step whose outputs
#: the summary reads.
SMOKE_STEP_ID = "consumer_smoke"
#: The fixed operation label the smoke's summary must carry.
SMOKE_OPERATION = "consumer_smoke"
#: The action step that turns the phase log into outputs.
REPORT_STEP = "Report the smoke phases"
SUFFIXES = ("outcome", "started-ns", "ended-ns")


@pytest.fixture(scope="module")
def release_workflow() -> dict[str, object]:
    """Load the release workflow once for the smoke wiring checks."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


@pytest.fixture(scope="module")
def smoke_action() -> dict[str, object]:
    """Load the consumer-smoke manifest once for the output checks."""
    return action_sources()[CONSUMER_SMOKE_ACTION_PATH]


def _expected_output(phase: str, suffix: str) -> str:
    """Return the expression reading one phase's output from the action.

    Returns
    -------
    str
        The GitHub expression the summary or the manifest must carry.
    """
    return (
        f"${{{{ steps.{SMOKE_STEP_ID}.outputs.{phase.replace('_', '-')}-{suffix} }}}}"
    )


def test_the_smoke_job_retains_phase_observability(
    release_workflow: dict[str, object],
) -> None:
    """Retain the smoke's phases in an always-run summary and JSONL artifact.

    The job runs only when the run is publishing, so the mode is fixed rather
    than read from an output that could disagree with the guard.
    """
    steps = job_steps(release_workflow, CONSUMER_SMOKE_JOB)
    env = assert_summary_environment(steps, SMOKE_OPERATION, "false", staged=False)
    assert_summary_phase_timings(env, SMOKE_PHASES)
    assert_retained_telemetry_policy(steps)
    for phase in SMOKE_PHASES:
        for suffix in SUFFIXES:
            key = f"{phase.upper()}_{suffix.upper().replace('-', '_')}"
            assert env.get(key) == _expected_output(phase, suffix), (
                f"the summary must read the {phase} {suffix} from the smoke action"
            )


def test_the_smoke_operation_reports_only_its_own_phases() -> None:
    """Keep the catalogue's smoke phases to the five the action performs.

    A phase the smoke does not run would be reported as skipped on every run,
    which reads as a failure that never happened, and a phase the catalogue
    lacks could not be summarised at all.
    """
    assert OPERATION_PHASES[SMOKE_OPERATION] == SMOKE_PHASES, (
        "the catalogue must name exactly the phases the smoke performs"
    )


def test_the_action_declares_every_phase_output(
    smoke_action: dict[str, object],
) -> None:
    """Require one output triple per phase, read from the report step.

    The verification step writes no outputs of its own: a refused smoke has
    still written its phase log, and the report step reads that log whatever
    the verification step's outcome was.
    """
    outputs = smoke_action.get("outputs")
    assert isinstance(outputs, dict), "the smoke action must declare outputs"
    expected = {
        f"{phase.replace('_', '-')}-{suffix}": (
            f"${{{{ steps.report.outputs.{phase.replace('_', '-')}-{suffix} }}}}"
        )
        for phase in SMOKE_PHASES
        for suffix in SUFFIXES
    }
    expected["verdict-path"] = "${{ steps.report.outputs.verdict-path }}"
    declared = {
        name: value.get("value")
        for name, value in outputs.items()
        if isinstance(value, dict)
    }
    assert declared == expected, (
        "the action must declare exactly the phase outputs the summary reads"
    )


def _report_run(smoke_action: dict[str, object]) -> object:
    """Return the report step's script.

    Returns
    -------
    object
        Whatever the step's ``run`` holds, so the checks below judge the
        document rather than a shape the test has already assumed.
    """
    runs = smoke_action.get("runs")
    assert isinstance(runs, dict), "the smoke action must declare runs"
    steps = [step for step in runs.get("steps", []) if isinstance(step, dict)]
    return named_step(steps, REPORT_STEP).get("run")


def _report_violations(run: object) -> list[str]:
    """Return how a report script fails to walk and emit every phase.

    The script is read as bash would run it, so a commented-out loop is
    documentation rather than a command, and a label list wrapped with
    continuations is one command rather than several.

    Returns
    -------
    list[str]
        One message per missing construct, or an empty list when the step
        walks the catalogue's phases in order and emits every suffix.
    """
    script = " ".join(" ".join(command_lines(run)).split())
    violations = []
    if f"for phase in {' '.join(SMOKE_PHASES)}; do" not in script:
        violations.append("the report step must walk exactly the catalogue's phases")
    violations.extend(
        f"the report step must emit each phase's {suffix}"
        for suffix in SUFFIXES
        if f"%s-{suffix}=%s" not in script
    )
    return violations


def test_the_report_step_walks_the_catalogue_phases_in_order(
    smoke_action: dict[str, object],
) -> None:
    """Require the report step's fixed label list to match the catalogue.

    The step reads a log the verifier writes and emits one triple per label.
    A label the catalogue names but the step omits would be summarised as
    skipped, and a label the step invented would be dropped by the reader, so
    the two lists must agree in order as well as in content.
    """
    assert _report_violations(_report_run(smoke_action)) == [], (
        "the report step must walk the catalogue's phases and emit each outcome"
    )


def test_a_commented_out_report_step_is_refused(
    smoke_action: dict[str, object],
) -> None:
    """Refuse a report step whose loop and output commands are comments.

    Prefixed with ``#`` the whole script is documentation: bash runs none of
    it, so the step emits nothing. A check reading raw shell text would still
    find both the loop and the format strings.
    """
    commented = "\n".join(
        f"# {line}" for line in str(_report_run(smoke_action)).splitlines()
    )
    assert _report_violations(commented) != [], (
        "a report step whose commands are all comments must not satisfy the contract"
    )


def test_the_smoke_targets_one_platform_named_by_its_archive() -> None:
    """Pin the target so the archive naming stays the one the release ships.

    The archive a consumer installs is named after this triple. A second
    target would need its own runner rather than another row here, so the
    value is a constant rather than a matrix.
    """
    assert SMOKE_ACTION_TARGET == "x86_64-unknown-linux-gnu", (
        "the smoke must exercise the release's Linux archive"
    )
