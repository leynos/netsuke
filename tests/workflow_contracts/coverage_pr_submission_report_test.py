"""Exercise the trusted PR coverage reporting seam and its publisher port."""

import functools
import importlib.util
import pathlib
import sys
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import types


ACTION_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_submission.py"
EXPECTED_COVERAGE_SUMMARY = (
    "Originating workflow run ID: 123\n"
    f"Originating commit SHA: {'a' * 40}\n"
    "Artifact name: pr-coverage-lcov\n"
    "Download outcome: success\n"
    "Download duration (ms): 12\n"
    "Validation outcome: success\n"
    "Validation duration (ms): 34\n"
    "Submission outcome: success\n"
    "Submission duration (ms): 56\n"
    "Conclusion: success"
)


@functools.cache
def _module(name: str, path: pathlib.Path) -> types.ModuleType:
    """Load one checked-in trusted Python module for direct behaviour tests."""
    specification = importlib.util.spec_from_file_location(name, path)
    assert specification is not None, f"{path.name} must be loadable"
    assert specification.loader is not None, f"{path.name} must have a loader"
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _action_module() -> types.ModuleType:
    """Return the trusted coverage action module."""
    return _module("coverage_pr_submission_test", ACTION_PATH)


def _environment(tmp_path: pathlib.Path) -> dict[str, str]:
    """Return fixed bounded workflow values for reporting behaviour tests."""
    return {
        "GITHUB_OUTPUT": str(tmp_path / "output"),
        "GITHUB_STEP_SUMMARY": str(tmp_path / "summary"),
        "ORIGINATING_WORKFLOW_RUN_ID": "123",
        "ORIGINATING_COMMIT_SHA": "a" * 40,
        "ARTIFACT_NAME": "pr-coverage-lcov",
        "ARTIFACT_DOWNLOAD_OUTCOME": "success",
        "ARTIFACT_VALIDATION_OUTCOME": "success",
        "SUBMISSION_OUTCOME": "success",
        "ARTIFACT_DOWNLOAD_DURATION_MS": "12",
        "ARTIFACT_VALIDATION_DURATION_MS": "34",
        "SUBMISSION_DURATION_MS": "56",
        "CHECK_RUN_PUBLICATION_OUTCOME": "success",
        "CHECK_RUN_PUBLICATION_DURATION_MS": "78",
        "CONCLUSION": "success",
    }


def test_start_telemetry_writes_the_injected_clock(tmp_path: pathlib.Path) -> None:
    """Write the supplied timestamp rather than reading a test process clock."""
    action = _action_module()
    environment = _environment(tmp_path)

    action.start_telemetry(environment, now_milliseconds=1234)

    assert (
        pathlib.Path(environment["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
        == "started_at_ms=1234\n"
    ), "the injected clock value must reach the step output"


def test_report_coverage_publishes_the_originating_commit(
    tmp_path: pathlib.Path,
) -> None:
    """Publish a bounded success Check Run through the injected publisher."""
    action = _action_module()
    environment = _environment(tmp_path)
    payloads: list[dict[str, object]] = []

    action.report_coverage(environment, payloads.append)

    assert len(payloads) == 1, "exactly one Check Run must be published"
    payload = payloads[0]
    assert payload["name"] == "CodeScene coverage", "the Check Run name is fixed"
    assert payload["head_sha"] == "a" * 40, "the originating commit is used"
    assert payload["external_id"] == "123", "the run ID is the idempotency key"
    assert payload["status"] == "completed", "the Check Run is published completed"
    assert payload["conclusion"] == "success", "a successful run stays successful"
    assert payload["output"] == {
        "title": "CodeScene coverage",
        "summary": EXPECTED_COVERAGE_SUMMARY,
    }, "only the bounded correlation summary is published"
    assert (
        pathlib.Path(environment["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
        == "conclusion=success\n"
    ), "the conclusion must reach the step output"


def test_report_excluded_fork_publishes_neutral_without_artifact_data(
    tmp_path: pathlib.Path,
) -> None:
    """Publish only a neutral Check Run for a fork excluded from submission."""
    action = _action_module()
    environment = _environment(tmp_path)
    payloads: list[dict[str, object]] = []

    action.report_excluded_fork(environment, payloads.append)

    payload = payloads[0]
    assert payload["conclusion"] == "neutral", "an excluded fork must not fail"
    assert payload["head_sha"] == "a" * 40, "the originating commit is still used"
    assert payload["external_id"] == "123", "the run ID is still the external ID"
    assert "Download outcome: skipped" in str(payload["output"]), (
        "the skipped download is reported"
    )
    assert "CS_ACCESS_TOKEN" not in str(payload), "no credential name is published"


def test_summarize_coverage_writes_only_bounded_fields(tmp_path: pathlib.Path) -> None:
    """Append trusted stage correlation without reading pull-request content."""
    action = _action_module()
    environment = _environment(tmp_path)

    action.summarize_coverage(environment)

    summary = pathlib.Path(environment["GITHUB_STEP_SUMMARY"]).read_text(
        encoding="utf-8"
    )
    assert "Originating workflow run ID: 123" in summary, "the run ID is summarized"
    assert f"Originating commit SHA: {'a' * 40}" in summary, "the commit is summarized"
    assert "Check Run publication duration (ms): 78" in summary, (
        "the publication duration is summarized"
    )
    assert "CS_ACCESS_TOKEN" not in summary, "no credential name is summarized"


@pytest.mark.parametrize(
    ("command", "expected_code"),
    [
        pytest.param(["start-telemetry"], 0, id="start-telemetry"),
        pytest.param(["report-coverage"], 0, id="report-coverage"),
        pytest.param(["report-excluded-fork"], 0, id="report-excluded-fork"),
        pytest.param(["summarize-coverage"], 0, id="summarize-coverage"),
    ],
)
def test_main_dispatches_public_workflow_commands(
    tmp_path: pathlib.Path, command: list[str], expected_code: int
) -> None:
    """Dispatch each public reporting command with controlled dependencies."""
    action = _action_module()
    environment = _environment(tmp_path)
    payloads: list[dict[str, object]] = []

    assert action.main(command, environment, payloads.append) == expected_code, (
        "each reporting command must succeed with a successful environment"
    )


def test_main_dispatches_archive_validation_with_its_argument(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> None:
    """Forward the archive directory only through the public validation command."""
    action = _action_module()
    received: list[str] = []

    def validate_artefact(artifact_directory: str) -> int:
        """Capture the command input through a fixed validator outcome seam."""
        received.append(artifact_directory)
        return 7

    monkeypatch.setattr(action, "validate_artefact", validate_artefact)

    assert (
        action.main(
            ["validate-artefact", "--artifact-directory", "coverage-artifact"],
            _environment(tmp_path),
        )
        == 7
    ), "the validator's own exit status must be forwarded"
    assert received == ["coverage-artifact"], "only the directory argument is passed"
