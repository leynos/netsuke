"""Exercise the trusted PR coverage handoff as an ordered workflow contract.

The pure JavaScript conclusion seam is checked through Node, the runtime used
by ``actions/github-script``. Keeping the outcome mapping outside the embedded
workflow script makes every success, neutral, and failure result executable in
tests without loading workflow event data or an environment.
"""

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

    ordered_steps = [
        DOWNLOAD_STEP,
        VALIDATION_STEP,
        SUBMISSION_STEP,
        REPORT_STEP,
        SUMMARY_STEP,
    ]
    indices = [unique_step_index(steps, name) for name in ordered_steps]
    assert indices == sorted(indices), "trusted coverage stages must remain ordered"


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
        "Conclusion",
    ):
        assert field in report_script, f"the Check Run summary must contain {field!r}"
        assert field in summary_script, f"the workflow summary must contain {field!r}"
    expected_report_environment = (
        ("SUBMISSION_OUTCOME", "${{ steps.submit_coverage.outcome }}"),
        ("ARTIFACT_DOWNLOAD_OUTCOME", "${{ steps.download_coverage.outcome }}"),
        ("ARTIFACT_VALIDATION_OUTCOME", "${{ steps.validate_coverage.outcome }}"),
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
