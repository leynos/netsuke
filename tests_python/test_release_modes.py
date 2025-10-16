"""Tests for the release mode helper used by the release workflow."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from importlib import util
from pathlib import Path
from typing import Dict

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "determine_release_modes.py"


@pytest.fixture(scope="module")
def release_modes_module():  # type: ignore[override]
    """Load the release mode helper as a Python module for unit tests."""

    spec = util.spec_from_file_location("determine_release_modes", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to load determine_release_modes module")
    module = util.module_from_spec(spec)
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        sys.modules.pop(spec.name, None)
        raise
    return module


class TestDetermineReleaseModes:
    """Unit tests for :func:`determine_release_modes`."""

    def test_push_event_publishes_and_uploads(self, release_modes_module):
        """Tag pushes should publish and upload artefacts."""

        modes = release_modes_module.determine_release_modes("push", {})
        assert modes.dry_run is False
        assert modes.should_publish is True
        assert modes.should_upload_workflow_artifacts is True

    def test_workflow_call_dry_run_disables_outputs(self, release_modes_module):
        """Dry-run invocations disable publishing and workflow artefacts."""

        event = {"inputs": {"dry-run": "true", "publish": "true"}}
        modes = release_modes_module.determine_release_modes("workflow_call", event)
        assert modes.dry_run is True
        assert modes.should_publish is False
        assert modes.should_upload_workflow_artifacts is False

    def test_invalid_bool_values_raise(self, release_modes_module):
        """Invalid boolean inputs should surface a helpful error."""

        event = {"inputs": {"dry-run": "maybe"}}
        with pytest.raises(ValueError, match="Cannot interpret 'maybe'"):
            release_modes_module.determine_release_modes("workflow_call", event)


class TestWorkflowBehaviour:
    """Behavioural tests exercising the script entry point."""

    def _run_helper(self, env: Dict[str, str]) -> Dict[str, str]:
        """Execute the helper script and capture its outputs."""

        result = subprocess.run(
            [sys.executable, str(SCRIPT_PATH)],
            check=True,
            capture_output=True,
            env=env,
            text=True,
        )
        assert result.stdout == ""
        outputs: Dict[str, str] = {}
        with Path(env["GITHUB_OUTPUT"]).open(encoding="utf-8") as handle:
            for line in handle:
                key, value = line.strip().split("=", 1)
                outputs[key] = value
        return outputs

    def test_dry_run_workflow_call_skips_uploads(self, tmp_path):
        """Dry-run workflow executions should skip uploading artefacts."""

        payload = {"inputs": {"dry-run": "true", "publish": "true"}}
        event_path = tmp_path / "event.json"
        event_path.write_text(json.dumps(payload), encoding="utf-8")
        output_path = tmp_path / "outputs.txt"

        env = os.environ.copy()
        env.update(
            {
                "GITHUB_EVENT_NAME": "workflow_call",
                "GITHUB_EVENT_PATH": str(event_path),
                "GITHUB_OUTPUT": str(output_path),
            }
        )

        outputs = self._run_helper(env)
        assert outputs["dry_run"] == "true"
        assert outputs["should_publish"] == "false"
        assert outputs["should_upload_workflow_artifacts"] == "false"

    def test_push_event_emits_publish_outputs(self, tmp_path):
        """Tag pushes should keep publishing and artefact uploads enabled."""

        event_path = tmp_path / "event.json"
        event_path.write_text("{}", encoding="utf-8")
        output_path = tmp_path / "outputs.txt"

        env = os.environ.copy()
        env.update(
            {
                "GITHUB_EVENT_NAME": "push",
                "GITHUB_EVENT_PATH": str(event_path),
                "GITHUB_OUTPUT": str(output_path),
            }
        )

        outputs = self._run_helper(env)
        assert outputs["dry_run"] == "false"
        assert outputs["should_publish"] == "true"
        assert outputs["should_upload_workflow_artifacts"] == "true"
