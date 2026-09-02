"""Contract tests wiring PR coverage generation to its isolated artefact.

``generate-coverage`` writes the report to ``output-path`` and the
the dedicated artefact upload step preserves it for the trusted submission
workflow. If the report path, artefact name, or step ordering drifts, the
submission cannot find validated coverage. These tests pin that unprivileged
wiring alongside the direct main-branch upload in ``coverage-main.yml``.
Shared parsing helpers live in ``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    job_steps,
    load_workflow,
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


def test_coverage_report_is_produced_before_artefact_upload() -> None:
    """The untrusted job uploads only the report generated after its tests.

    The coverage step runs after ``make test`` so the report reflects the
    tested tree, and the CodeScene check runs after the coverage step so the
    report exists before the narrow artefact upload starts.
    """
    steps = job_steps(load_workflow(), "build-test")
    test_index = unique_step_index(steps, "Test")
    coverage_index = unique_step_index(steps, COVERAGE_STEP)
    artefact_index = unique_step_index(steps, PR_COVERAGE_ARTEFACT_STEP)
    assert test_index < coverage_index < artefact_index, (
        "the build-test job must run Test, then coverage, then its artefact "
        f"upload; got indices {test_index}, {coverage_index}, {artefact_index}"
    )

    _assert_with_inputs(
        steps[coverage_index],
        COVERAGE_STEP,
        {"language": "rust", "output-path": "lcov.info", "format": "lcov"},
    )
    _assert_with_inputs(
        steps[artefact_index],
        PR_COVERAGE_ARTEFACT_STEP,
        {"name": "pr-coverage-lcov", "path": "lcov.info", "retention-days": 3},
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
