"""Tests for the release mode helper used by the release workflow."""

from __future__ import annotations

import json
import sys
from collections.abc import Iterator
from dataclasses import dataclass
from importlib import util
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "determine_release_modes.py"


@dataclass(frozen=True)
class WorkflowTestCase:
    """Test case data for workflow behaviour tests."""

    event_name: str
    payload: dict[str, object]
    expected: dict[str, str]


@pytest.fixture(scope="module")
def release_modes_module() -> Iterator[ModuleType]:
    """Load the release mode helper as a Python module for unit tests."""

    spec = util.spec_from_file_location("determine_release_modes", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        message = "Unable to load determine_release_modes module"
        raise RuntimeError(message)
    module = util.module_from_spec(spec)
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
        yield module
    finally:
        sys.modules.pop(spec.name, None)


class TestDetermineReleaseModes:
    """Unit tests for :func:`determine_release_modes`."""

    def test_push_event_publishes_and_uploads(self, release_modes_module):
        """Tag pushes should publish and upload artefacts."""

        modes = release_modes_module.determine_release_modes("push", {})
        assert modes.dry_run is False, "Push events must not enter dry-run mode"
        assert modes.should_publish is True, "Push events must request publishing"
        assert (
            modes.should_upload_workflow_artifacts is True
        ), "Push events must upload workflow artefacts"

    def test_workflow_call_dry_run_disables_outputs(self, release_modes_module):
        """Dry-run invocations disable publishing and workflow artefacts."""

        event = {"inputs": {"dry-run": "true", "publish": "true"}}
        modes = release_modes_module.determine_release_modes("workflow_call", event)
        assert (
            modes.dry_run is True
        ), "Dry-run invocations must enable dry-run mode"
        assert (
            modes.should_publish is False
        ), "Dry-run invocations must suppress publishing"
        assert (
            modes.should_upload_workflow_artifacts is False
        ), "Dry-run invocations must suppress workflow artefact uploads"

    def test_invalid_bool_values_raise(self, release_modes_module):
        """Invalid boolean inputs should surface a helpful error."""

        event = {"inputs": {"dry-run": "maybe"}}
        with pytest.raises(ValueError, match="Cannot interpret 'maybe'"):
            release_modes_module.determine_release_modes("workflow_call", event)

    def test_unsupported_event_raises(self, release_modes_module):
        """Unsupported events should raise a clear error."""

        with pytest.raises(ValueError, match="Unsupported event 'schedule'"):
            release_modes_module.determine_release_modes("schedule", {})

    def test_pull_request_defaults_to_dry_run(self, release_modes_module):
        """Pull request events should default to dry-run mode."""

        modes = release_modes_module.determine_release_modes("pull_request", {})
        assert modes.dry_run is True, "Pull requests must enable dry-run mode"
        assert (
            modes.should_publish is False
        ), "Pull requests must suppress publishing"
        assert (
            modes.should_upload_workflow_artifacts is False
        ), "Pull requests must skip workflow artefact uploads"


class TestWorkflowBehaviour:
    """Behavioural tests exercising the script entry point."""

    @staticmethod
    def _invoke_helper(
        module: ModuleType,
        *,
        env: dict[str, str],
        output_path: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> dict[str, str]:
        """Execute ``main`` under a controlled environment."""

        for key, value in env.items():
            monkeypatch.setenv(key, value)

        module.main()

        outputs: dict[str, str] = {}
        with output_path.open(encoding="utf-8") as handle:
            for line in handle:
                key, value = line.strip().split("=", 1)
                outputs[key] = value
        return outputs

    @pytest.mark.parametrize(
        "test_case",
        [
            WorkflowTestCase(
                event_name="workflow_call",
                payload={"inputs": {"dry-run": "true", "publish": "true"}},
                expected={
                    "dry_run": "true",
                    "should_publish": "false",
                    "should_upload_workflow_artifacts": "false",
                },
            ),
            WorkflowTestCase(
                event_name="push",
                payload={},
                expected={
                    "dry_run": "false",
                    "should_publish": "true",
                    "should_upload_workflow_artifacts": "true",
                },
            ),
            WorkflowTestCase(
                event_name="pull_request",
                payload={},
                expected={
                    "dry_run": "true",
                    "should_publish": "false",
                    "should_upload_workflow_artifacts": "false",
                },
            ),
        ],
    )
    def test_entry_point_outputs(
        self,
        test_case: WorkflowTestCase,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        release_modes_module: ModuleType,
    ) -> None:
        """Executing the helper emits workflow outputs for the caller."""

        event_path = tmp_path / "event.json"
        event_path.write_text(json.dumps(test_case.payload), encoding="utf-8")
        output_path = tmp_path / "outputs.txt"

        outputs = self._invoke_helper(
            release_modes_module,
            env={
                "GITHUB_EVENT_NAME": test_case.event_name,
                "GITHUB_EVENT_PATH": str(event_path),
                "GITHUB_OUTPUT": str(output_path),
            },
            output_path=output_path,
            monkeypatch=monkeypatch,
        )

        assert (
            outputs == test_case.expected
        ), "Script outputs must match the expected mapping"
