"""Contract tests wiring coverage generation to the CodeScene gates.

``generate-coverage`` writes the report to ``output-path`` and the
``upload-codescene-coverage`` steps read that exact path in lcov format. If
the report path, the format, or the step ordering drifts, CodeScene reports
"No valid coverage report found in the build pipeline" — a failure with no
pointer back to the edit that caused it. These tests pin the wiring on both
the pull-request workflow (``ci.yml``) and the main-branch workflow
(``coverage-main.yml``). Shared parsing helpers live in
``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
)

COVERAGE_STEP = "Test and Measure Coverage"
CODESCENE_CHECK_STEP = "Check coverage against CodeScene gates"
CODESCENE_UPLOAD_STEP = "Upload coverage data to CodeScene"

GENERATE_COVERAGE_ACTION = "leynos/shared-actions/.github/actions/generate-coverage@"
UPLOAD_COVERAGE_ACTION = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
)


def _step_index(steps: list[dict[str, object]], name: str) -> int:
    """Return the position of the uniquely named step."""
    names = [step.get("name") for step in steps]
    assert names.count(name) == 1, (
        f"expected exactly one step named {name!r}, got step names {names!r}"
    )
    return names.index(name)


def _assert_with_inputs(
    step: dict[str, object], description: str, expected: dict[str, object]
) -> None:
    """Assert a step's ``with`` block carries exactly the expected inputs."""
    with_ = require_mapping(step.get("with"), f"{description}'s with block")
    actual = {key: with_.get(key) for key in expected}
    assert actual == expected, f"{description} must pass {expected!r}, got {actual!r}"


def test_coverage_report_is_produced_before_codescene_check() -> None:
    """The CodeScene gate consumes the report the coverage step produces.

    The coverage step runs after ``make test`` so the report reflects the
    tested tree, and the CodeScene check runs after the coverage step so the
    report exists in the build pipeline by the time it is read.
    """
    steps = job_steps(load_workflow(), "build-test")
    test_index = _step_index(steps, "Test")
    coverage_index = _step_index(steps, COVERAGE_STEP)
    codescene_index = _step_index(steps, CODESCENE_CHECK_STEP)
    assert test_index < coverage_index < codescene_index, (
        "the build-test job must run Test, then coverage, then the CodeScene "
        f"check; got indices {test_index}, {coverage_index}, {codescene_index}"
    )

    _assert_with_inputs(
        steps[coverage_index],
        COVERAGE_STEP,
        {"output-path": "lcov.info", "format": "lcov"},
    )
    _assert_with_inputs(
        steps[codescene_index],
        CODESCENE_CHECK_STEP,
        {"path": "lcov.info", "format": "lcov", "mode": "check"},
    )


def test_main_coverage_upload_reads_the_generated_lcov_report() -> None:
    """Main uploads the LCOV report it produces before calling CodeScene."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), "coverage-upload")
    coverage_index = _step_index(steps, COVERAGE_STEP)
    upload_index = _step_index(steps, CODESCENE_UPLOAD_STEP)
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
        {"output-path": "lcov.info", "format": "lcov"},
    )
    _assert_with_inputs(
        upload_step,
        "main CodeScene upload",
        {"path": "lcov.info", "format": "lcov"},
    )
