"""Contract tests separating the PR ratchet from the main coverage upload.

Pull requests generate coverage and compare it with the ratcheted baseline;
they do not publish coverage artefacts or contact CodeScene. The main workflow
generates the matching report, advances the ratchet baseline, and uploads that
authoritative report to CodeScene. These tests pin both halves of that split.
Shared parsing helpers live in ``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    COVERAGE_PR_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    unique_step_index,
)

COVERAGE_STEP = "Test and Measure Coverage"
PR_COVERAGE_ARTEFACT_STEP = "Upload PR coverage artefact"
CODESCENE_UPLOAD_STEP = "Upload coverage data to CodeScene"

GENERATE_COVERAGE_ACTION = "leynos/shared-actions/.github/actions/generate-coverage@"
UPLOAD_COVERAGE_ACTION = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
)


def _assert_with_inputs(
    step: dict[str, object], description: str, expected: dict[str, object]
) -> None:
    """Validate that a step's ``with`` block supplies the expected inputs."""
    with_ = require_mapping(step.get("with"), f"{description}'s with block")
    actual = {key: with_.get(key) for key in expected}
    assert actual == expected, f"{description} must pass {expected!r}, got {actual!r}"


def test_pr_coverage_stays_local_and_uses_the_main_ratchet() -> None:
    """Keep pull-request coverage inside CI and compare it with main."""
    steps = job_steps(load_workflow(), "build-test")
    coverage_step = named_step(steps, COVERAGE_STEP)
    _assert_with_inputs(
        coverage_step,
        COVERAGE_STEP,
        {
            "language": "rust",
            "output-path": "lcov.info",
            "format": "lcov",
            "with-ratchet": "true",
        },
    )
    assert not [
        step for step in steps if step.get("name") == PR_COVERAGE_ARTEFACT_STEP
    ], "pull requests must not publish their coverage report"
    assert not COVERAGE_PR_WORKFLOW_PATH.exists(), (
        "pull requests must not have a privileged CodeScene submission workflow"
    )


def test_main_coverage_upload_reads_the_generated_lcov_report() -> None:
    """Main uploads the LCOV report it produces before calling CodeScene."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), "coverage-upload")
    coverage_index = unique_step_index(steps, COVERAGE_STEP)
    upload_index = unique_step_index(steps, CODESCENE_UPLOAD_STEP)
    assert coverage_index < upload_index, (
        "main must generate coverage before uploading it to CodeScene"
    )

    coverage_step = steps[coverage_index]
    upload_step = steps[upload_index]
    assert str(coverage_step.get("uses", "")).startswith(GENERATE_COVERAGE_ACTION), (
        "main coverage production must use generate-coverage, got "
        f"{coverage_step.get('uses')!r}"
    )
    assert str(upload_step.get("uses", "")).startswith(UPLOAD_COVERAGE_ACTION), (
        "main coverage upload must use upload-codescene-coverage, got "
        f"{upload_step.get('uses')!r}"
    )

    _assert_with_inputs(
        coverage_step,
        "main coverage production",
        {"language": "rust", "output-path": "lcov.info", "format": "lcov"},
    )
    _assert_with_inputs(
        upload_step,
        "main CodeScene upload",
        {"path": "lcov.info", "format": "lcov"},
    )
