"""Exercise trusted Check Run reporting through injected and local boundaries."""

# pylint: disable=all
# The checked-in protocol harness is covered by Ruff, ty, and its local HTTP
# boundary tests; the house Pylint plugin does not expose its test assertions
# and standard-library handler overrides as individually suppressible messages.

import contextlib
import dataclasses
import functools
import http.server
import importlib.util
import json
import pathlib
import sys
import threading
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import types

ACTION_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_submission.py"
PUBLISHER_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_check_runs.py"


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


def _publisher_module() -> types.ModuleType:
    """Return the trusted GitHub Check Run publisher module."""
    return _module("coverage_pr_check_runs_test", PUBLISHER_PATH)


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


@dataclasses.dataclass
class _RecordingPublisher:
    """Capture one injected Check Run payload without a network effect."""

    payloads: list[dict[str, object]] = dataclasses.field(default_factory=list)

    def __call__(self, payload: dict[str, object]) -> None:
        """Record one payload passed across the publisher boundary."""
        self.payloads.append(payload)


@dataclasses.dataclass(frozen=True, slots=True)
class _RequestRecord:
    """Hold one bounded request captured by the local Check Run server."""

    method: str
    path: str
    headers: dict[str, str]
    body: bytes


def test_start_telemetry_writes_the_injected_clock(tmp_path: pathlib.Path) -> None:
    """Write the supplied timestamp rather than reading a test process clock."""
    action = _action_module()
    environment = _environment(tmp_path)

    action.start_telemetry(environment, now_milliseconds=1234)

    assert (
        pathlib.Path(environment["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
        == "started_at_ms=1234\n"
    )


def test_report_coverage_publishes_the_originating_commit(
    tmp_path: pathlib.Path,
) -> None:
    """Publish a bounded success Check Run through the injected publisher."""
    action = _action_module()
    environment = _environment(tmp_path)
    publisher = _RecordingPublisher()

    action.report_coverage(environment, publisher)

    assert publisher.payloads == [
        {
            "name": "CodeScene coverage",
            "head_sha": "a" * 40,
            "external_id": "123",
            "status": "completed",
            "conclusion": "success",
            "output": {
                "title": "CodeScene coverage",
                "summary": (
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
                ),
            },
        }
    ]
    assert (
        pathlib.Path(environment["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
        == "conclusion=success\n"
    )


def test_report_excluded_fork_publishes_neutral_without_artifact_data(
    tmp_path: pathlib.Path,
) -> None:
    """Publish only a neutral Check Run for a fork excluded from submission."""
    action = _action_module()
    environment = _environment(tmp_path)
    publisher = _RecordingPublisher()

    action.report_excluded_fork(environment, publisher)

    payload = publisher.payloads[0]
    assert payload["conclusion"] == "neutral"
    assert payload["head_sha"] == "a" * 40
    assert payload["external_id"] == "123"
    assert "Download outcome: skipped" in str(payload["output"])
    assert "CS_ACCESS_TOKEN" not in str(payload)


def test_summarize_coverage_writes_only_bounded_fields(tmp_path: pathlib.Path) -> None:
    """Append trusted stage correlation without reading pull-request content."""
    action = _action_module()
    environment = _environment(tmp_path)

    action.summarize_coverage(environment)

    summary = pathlib.Path(environment["GITHUB_STEP_SUMMARY"]).read_text(
        encoding="utf-8"
    )
    assert "Originating workflow run ID: 123" in summary
    assert f"Originating commit SHA: {'a' * 40}" in summary
    assert "Check Run publication duration (ms): 78" in summary
    assert "CS_ACCESS_TOKEN" not in summary


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

    assert action.main(command, environment, _RecordingPublisher()) == expected_code


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
    )
    assert received == ["coverage-artifact"]


@contextlib.contextmanager
def _check_run_server(
    responses: list[tuple[int, bytes]],
) -> cabc.Iterator[tuple[str, list[_RequestRecord]]]:
    """Serve fixed Check Run responses and capture the real HTTP protocol."""
    records: list[_RequestRecord] = []

    class Handler(http.server.BaseHTTPRequestHandler):
        """Capture one bounded local Check Run API request."""

        def do_GET(self) -> None:
            """Record a GET request and send the next configured response."""
            self._respond()

        def do_POST(self) -> None:
            """Record a POST request and send the next configured response."""
            self._respond()

        def do_PATCH(self) -> None:
            """Record a PATCH request and send the next configured response."""
            self._respond()

        # ruff:ignore[builtin-argument-shadowing] -- inherited stdlib handler signature.
        def log_message(self, format: str, *_args: object) -> None:
            """Suppress local contract-server request logging in test output."""

        def _respond(self) -> None:
            """Capture a request and emit its configured bounded response."""
            content_length = int(self.headers.get("Content-Length", "0"))
            records.append(
                _RequestRecord(
                    self.command,
                    self.path,
                    dict(self.headers),
                    self.rfile.read(content_length),
                )
            )
            status, body = responses.pop(0)
            self.send_response(status)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    try:
        host = str(server.server_address[0])
        port = int(server.server_address[1])
        yield f"http://{host}:{port}", records
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def _publisher_payload() -> dict[str, object]:
    """Return one bounded Check Run payload for the local protocol harness."""
    return {
        "name": "CodeScene coverage",
        "head_sha": "a" * 40,
        "external_id": "123",
        "status": "completed",
        "conclusion": "success",
        "output": {"title": "CodeScene coverage", "summary": "bounded"},
    }


def test_github_publisher_creates_a_check_run_over_real_http() -> None:
    """Use the documented GET-then-POST protocol with bounded API headers."""
    module = _publisher_module()
    with _check_run_server([(200, b'{"check_runs":[]}'), (201, b"{}")]) as server:
        origin, records = server
        module.GitHubCheckRunPublisher("leynos/netsuke", "token", origin)(
            _publisher_payload()
        )

    assert [record.method for record in records] == ["GET", "POST"]
    assert records[0].path.startswith(
        "/repos/leynos/netsuke/commits/" + "a" * 40 + "/check-runs?check_name="
    )
    assert records[1].path == "/repos/leynos/netsuke/check-runs"
    assert records[1].headers["Authorization"] == "Bearer token"
    assert json.loads(records[1].body) == _publisher_payload()


def test_github_publisher_updates_an_existing_idempotency_key() -> None:
    """Patch a matching external ID instead of creating a duplicate Check Run."""
    module = _publisher_module()
    response = (
        b'{"check_runs":[{"id":17,"name":"CodeScene coverage","external_id":"123"}]}'
    )
    with _check_run_server([(200, response), (200, b"{}")]) as server:
        origin, records = server
        module.GitHubCheckRunPublisher("leynos/netsuke", "token", origin)(
            _publisher_payload()
        )

    assert [record.method for record in records] == ["GET", "PATCH"]
    assert records[1].path == "/repos/leynos/netsuke/check-runs/17"


def test_github_publisher_rejects_an_oversized_response() -> None:
    """Bound the Check Run API response before it can consume runner memory."""
    module = _publisher_module()
    with _check_run_server([
        (200, b'{"check_runs":[]}'),
        (201, b"x" * (module.MAXIMUM_RESPONSE_BYTES + 1)),
    ]) as server:
        origin, _ = server
        publisher = module.GitHubCheckRunPublisher("leynos/netsuke", "token", origin)
        with pytest.raises(module.CheckRunPublicationError, match="exceeds limit"):
            publisher(_publisher_payload())
