"""Exercise the trusted PR coverage handoff as an ordered workflow contract.

The pure JavaScript conclusion seam is checked through Node, the runtime used
by ``actions/github-script``. Keeping the outcome mapping outside the embedded
workflow script makes every success, neutral, and failure result executable in
tests without loading workflow event data or an environment.
"""

import copy
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - invokes the checked-in Node seam.

import pytest
from workflow_loading import (
    COVERAGE_PR_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    unique_step_index,
)

OUTCOME_MODULE_PATH = (
    REPO_ROOT / ".github" / "scripts" / "codescene-coverage-outcome.js"
)
ARTEFACT_NAME = "pr-coverage-lcov"
DOWNLOAD_STEP = "Download PR coverage artefact"
VALIDATION_STEP = "Validate hostile coverage artefact"
SUBMISSION_STEP = "Check coverage against CodeScene gates"
REPORT_STEP = "Report CodeScene coverage gate"
SUMMARY_STEP = "Summarize CodeScene coverage gate"
TRUSTED_CHECKOUT_STEP = "Check out trusted validation tooling"
SETUP_UV_STEP = "Setup uv"
TRUSTED_CHECKOUT_ACTION = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
SETUP_UV_ACTION = "astral-sh/setup-uv@11f9893b081a58869d3b5fccaea48c9e9e46f990"
DOWNLOAD_ACTION = "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c"
SUBMISSION_ACTION = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
    "32c8ea649ea44d40119f348ad48861212532061f"
)
GITHUB_SCRIPT_ACTION = "actions/github-script@ed597411d8f924073f98dfc5c65a23a2325f34cd"
VALIDATION_COMMAND = (
    "make validate-coverage-artifact COVERAGE_ARTIFACT_DIR=coverage-artifact"
)
EXPECTED_DOWNLOAD_INPUTS = {
    "name": ARTEFACT_NAME,
    "run-id": "${{ github.event.workflow_run.id }}",
    "github-token": "${{ github.token }}",
    "path": "coverage-artifact",
}
EXPECTED_SUBMISSION_INPUTS = {
    "path": "coverage-artifact/lcov.info",
    "format": "lcov",
    "mode": "check",
    "project-url": "https://api.codescene.io/v2/projects/69281",
    "access-token": "${{ env.CS_ACCESS_TOKEN }}",
    "installer-checksum": "${{ vars.CODESCENE_CLI_SHA256 }}",
}


def _coverage_conclusion(
    download_outcome: str,
    validation_outcome: str,
    submission_outcome: str,
) -> str:
    """Return the checked-in JavaScript conclusion for three stage outcomes."""
    script = (
        "const { coverageConclusion } = require(process.argv[1]);"
        "process.stdout.write(coverageConclusion(process.argv[2], process.argv[3], "
        "process.argv[4]));"
    )
    node = shutil.which("node")
    assert node is not None, "Node is required by the github-script action runtime"
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
        [
            node,
            "--eval",
            script,
            "--",
            str(OUTCOME_MODULE_PATH),
            download_outcome,
            validation_outcome,
            submission_outcome,
        ],
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    return result.stdout


def _assert_submission_mechanics(steps: list[dict[str, object]]) -> None:
    """Assert the pinned data-validation and CodeScene submission contract."""
    checkout = named_step(steps, TRUSTED_CHECKOUT_STEP)
    setup_uv = named_step(steps, SETUP_UV_STEP)
    download = named_step(steps, DOWNLOAD_STEP)
    validation = named_step(steps, VALIDATION_STEP)
    submission = named_step(steps, SUBMISSION_STEP)
    report = named_step(steps, REPORT_STEP)

    assert checkout.get("uses") == TRUSTED_CHECKOUT_ACTION, (
        "trusted checkout must remain pinned"
    )
    assert setup_uv.get("uses") == SETUP_UV_ACTION, (
        "the uv setup action must remain pinned"
    )
    assert download.get("uses") == DOWNLOAD_ACTION, (
        "hostile artefact download must remain pinned"
    )
    assert validation.get("run") == VALIDATION_COMMAND, (
        "the hostile artefact must pass the exact validation gate"
    )
    assert submission.get("uses") == SUBMISSION_ACTION, (
        "CodeScene submission must remain pinned to the reviewed action"
    )
    assert require_mapping(submission.get("with"), "submission inputs") == (
        EXPECTED_SUBMISSION_INPUTS
    ), "CodeScene submission must retain its reviewed data-only inputs"
    assert report.get("uses") == GITHUB_SCRIPT_ACTION, (
        "the trusted Check Run reporter must remain pinned"
    )


@pytest.mark.parametrize(
    ("download_outcome", "validation_outcome", "submission_outcome", "expected"),
    [
        pytest.param("success", "success", "success", "success", id="success"),
        pytest.param(
            "success",
            "success",
            "skipped",
            "neutral",
            id="absent-token",
        ),
        pytest.param("failure", "skipped", "skipped", "failure", id="download-failure"),
        pytest.param(
            "success", "failure", "skipped", "failure", id="validation-failure"
        ),
        pytest.param(
            "success", "success", "failure", "failure", id="submission-failure"
        ),
    ],
)
def test_coverage_conclusion_preserves_stage_outcomes(
    download_outcome: str,
    validation_outcome: str,
    submission_outcome: str,
    expected: str,
) -> None:
    """Map every trusted-handoff terminal state to its Check Run conclusion."""
    assert (
        _coverage_conclusion(download_outcome, validation_outcome, submission_outcome)
        == expected
    ), "the Check Run conclusion must match its three stage outcomes"


def test_submission_workflow_orders_the_hostile_data_handoff() -> None:
    """Download, validate, submit, report, and summarize in that strict order."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = job_steps(workflow, "submit-coverage")
    download = named_step(steps, DOWNLOAD_STEP)
    download_with = require_mapping(download.get("with"), "coverage download inputs")
    assert download_with["name"] == ARTEFACT_NAME, (
        "the trusted workflow must download only the fixed coverage artefact"
    )
    assert download_with["run-id"] == "${{ github.event.workflow_run.id }}", (
        "the download must be correlated to the source workflow run"
    )
    assert "skip-decompress" not in download_with, (
        "the validator must receive the extracted LCOV data file"
    )
    assert download_with == EXPECTED_DOWNLOAD_INPUTS, (
        "the download action must receive only its reviewed cross-run inputs"
    )
    _assert_submission_mechanics(steps)

    ordered_steps = [
        DOWNLOAD_STEP,
        VALIDATION_STEP,
        SUBMISSION_STEP,
        REPORT_STEP,
        SUMMARY_STEP,
    ]
    indices = [unique_step_index(steps, name) for name in ordered_steps]
    assert indices == sorted(indices), "trusted coverage stages must remain ordered"


@pytest.mark.parametrize("target", [VALIDATION_STEP, SUBMISSION_STEP])
def test_submission_workflow_rejects_noop_security_stages(target: str) -> None:
    """Reject mutations that replace validation or submission with a no-op."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = copy.deepcopy(job_steps(workflow, "submit-coverage"))
    mutated_step = named_step(steps, target)
    if target == VALIDATION_STEP:
        mutated_step["run"] = "true"
    else:
        mutated_step["uses"] = GITHUB_SCRIPT_ACTION
        mutated_step["with"] = {"script": ""}

    with pytest.raises(AssertionError):
        _assert_submission_mechanics(steps)


def test_check_run_and_summary_publish_only_bounded_correlation() -> None:
    """Report the source run and stage outcomes without untrusted PR content."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = job_steps(workflow, "submit-coverage")
    report = named_step(steps, REPORT_STEP)
    report_script = str(require_mapping(report.get("with"), "report inputs")["script"])
    report_environment = require_mapping(report.get("env"), "report environment")
    summary = named_step(steps, SUMMARY_STEP)
    summary_script = str(summary["run"])
    summary_environment = require_mapping(summary.get("env"), "summary environment")

    for required_fragment in (
        "head_sha: context.payload.workflow_run.head_sha",
        "external_id: workflowRunId",
        "core.setOutput('conclusion', conclusion)",
    ):
        assert required_fragment in report_script, (
            f"the Check Run report must contain {required_fragment!r}"
        )
    for field in (
        "Originating workflow run ID",
        "Originating commit SHA",
        "Artifact name",
        "Download outcome",
        "Validation outcome",
        "Submission outcome",
        "Download duration (ms)",
        "Validation duration (ms)",
        "Submission duration (ms)",
        "Conclusion",
    ):
        assert field in report_script, f"the Check Run summary must contain {field!r}"
        assert field in summary_script, f"the workflow summary must contain {field!r}"
    expected_report_environment = (
        ("SUBMISSION_OUTCOME", "${{ steps.submit_coverage.outcome }}"),
        ("ARTIFACT_DOWNLOAD_OUTCOME", "${{ steps.download_coverage.outcome }}"),
        ("ARTIFACT_VALIDATION_OUTCOME", "${{ steps.validate_coverage.outcome }}"),
        (
            "ARTIFACT_DOWNLOAD_DURATION_MS",
            "${{ steps.observe_download_coverage.outputs.duration_ms }}",
        ),
        (
            "ARTIFACT_VALIDATION_DURATION_MS",
            "${{ steps.observe_validate_coverage.outputs.duration_ms }}",
        ),
        (
            "SUBMISSION_DURATION_MS",
            "${{ steps.observe_submit_coverage.outputs.duration_ms }}",
        ),
        ("ORIGINATING_WORKFLOW_RUN_ID", "${{ github.event.workflow_run.id }}"),
        ("ORIGINATING_COMMIT_SHA", "${{ github.event.workflow_run.head_sha }}"),
        ("ARTIFACT_NAME", ARTEFACT_NAME),
    )
    assert set(report_environment) == {
        name for name, _ in expected_report_environment
    }, "the Check Run must receive only its bounded correlation fields"
    for name, expected_value in expected_report_environment:
        assert report_environment[name] == expected_value, (
            f"the Check Run must receive the expected {name} correlation value"
        )
    assert summary_environment["ORIGINATING_WORKFLOW_RUN_ID"] == (
        "${{ github.event.workflow_run.id }}"
    ), "the workflow summary must retain the source run ID"
    assert summary_environment["ORIGINATING_COMMIT_SHA"] == (
        "${{ github.event.workflow_run.head_sha }}"
    ), "the workflow summary must retain the source commit SHA"
    assert summary_environment["ARTIFACT_NAME"] == ARTEFACT_NAME, (
        "the workflow summary must retain the fixed artefact name"
    )


def test_telemetry_uses_fixed_operations_and_bounded_stage_fields() -> None:
    """Record each handoff stage with duration and trusted correlation data."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = job_steps(workflow, "submit-coverage")
    telemetry = (
        (
            "Record coverage artefact download telemetry",
            "coverage-artifact-download",
            "${{ steps.download_coverage.outcome }}",
        ),
        (
            "Record hostile coverage validation telemetry",
            "hostile-coverage-validation",
            "${{ steps.validate_coverage.outcome }}",
        ),
        (
            "Record CodeScene submission telemetry",
            "codescene-submission",
            "${{ steps.submit_coverage.outcome }}",
        ),
    )
    for name, operation, outcome in telemetry:
        step = named_step(steps, name)
        environment = require_mapping(step.get("env"), f"{name} environment")
        script = str(step["run"])
        assert environment["OUTCOME"] == outcome, (
            f"{name} must report only its own stage outcome"
        )
        assert environment["ORIGINATING_WORKFLOW_RUN_ID"] == (
            "${{ github.event.workflow_run.id }}"
        ), f"{name} must retain source-run correlation"
        assert environment["ORIGINATING_COMMIT_SHA"] == (
            "${{ github.event.workflow_run.head_sha }}"
        ), f"{name} must retain source-commit correlation"
        assert f"operation={operation}" in script, (
            f"{name} must use its fixed operation name"
        )
        assert "duration_ms=" in script, f"{name} must emit a duration metric"


def test_observability_avoids_sensitive_or_untrusted_fields() -> None:
    """Forbid secrets, shell state, and PR-controlled metadata in status output."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = job_steps(workflow, "submit-coverage")
    report = named_step(steps, REPORT_STEP)
    summary = named_step(steps, SUMMARY_STEP)
    observable_values = "\n".join([
        str(require_mapping(report.get("with"), "report inputs")["script"]),
        str(require_mapping(report.get("env"), "report environment")),
        str(summary["run"]),
        str(require_mapping(summary.get("env"), "summary environment")),
        *[
            str(named_step(steps, name)["run"])
            for name in (
                "Record coverage artefact download telemetry",
                "Record hostile coverage validation telemetry",
                "Record CodeScene submission telemetry",
            )
        ],
    ])

    for forbidden in (
        "CS_ACCESS_TOKEN",
        "secrets.",
        "BASH_ENV",
        "GITHUB_PATH",
        "pull_request.title",
        "head_ref",
        "head_branch",
    ):
        assert forbidden not in observable_values, (
            f"observability output must not contain {forbidden!r}"
        )
