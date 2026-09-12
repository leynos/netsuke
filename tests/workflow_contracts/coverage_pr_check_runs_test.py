"""Exercise trusted Check Run publication through its transport port.

``coverage_pr_check_runs.py`` owns the one adapter that reaches GitHub, so
these tests drive it twice: through a recording transport injected at the
publication boundary, and against a loopback server that speaks the real HTTP
protocol the fixed Check Run requests must survive.
"""

import contextlib
import dataclasses
import functools
import http.server
import importlib.util
import json
import sys
import threading
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import types

    class _BoundedRequest(typ.Protocol):
        """Describe the bounded request fields carried across the transport port."""

        method: str
        path: str
        payload: cabc.Mapping[str, object] | None


PUBLISHER_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_check_runs.py"


@functools.cache
def _publisher_module() -> types.ModuleType:
    """Load the checked-in trusted GitHub Check Run publisher module."""
    specification = importlib.util.spec_from_file_location(
        "coverage_pr_check_runs_test", PUBLISHER_PATH
    )
    assert specification is not None, "the publisher module must be loadable"
    assert specification.loader is not None, "the publisher module must have a loader"
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


@dataclasses.dataclass(slots=True)
class _RecordingTransport:
    """Record bounded requests crossing the publication transport port."""

    responses: list[bytes] = dataclasses.field(default_factory=list)
    requests: list[_BoundedRequest] = dataclasses.field(default_factory=list)

    def __call__(self, request: _BoundedRequest) -> bytes:
        """Record one bounded request and return its configured response."""
        self.requests.append(request)
        return self.responses.pop(0)


@dataclasses.dataclass(frozen=True, slots=True)
class _RequestRecord:
    """Hold one bounded request captured by the local Check Run server."""

    method: str
    path: str
    headers: dict[str, str]
    body: bytes


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


@pytest.mark.parametrize(
    ("lookup_response", "expected_method", "expected_path"),
    [
        pytest.param(
            b'{"total_count":0,"check_runs":[]}',
            "POST",
            "/repos/leynos/netsuke/check-runs",
            id="create",
        ),
        pytest.param(
            b'{"total_count":1,"check_runs":[{"id":17,'
            b'"name":"CodeScene coverage","external_id":"123"}]}',
            "PATCH",
            "/repos/leynos/netsuke/check-runs/17",
            id="update",
        ),
    ],
)
def test_publish_selects_its_method_through_the_transport_port(
    lookup_response: bytes, expected_method: str, expected_path: str
) -> None:
    """Cross one narrow injected transport for both lookup and publication."""
    module = _publisher_module()
    transport = _RecordingTransport(responses=[lookup_response, b"{}"])
    publisher = module.GitHubCheckRunPublisher("leynos/netsuke", transport)

    publisher.publish(_publisher_payload())

    assert [request.method for request in transport.requests] == [
        "GET",
        expected_method,
    ], "publication must look up its idempotency key through the same port"
    assert transport.requests[0].payload is None, "the lookup read carries no payload"
    assert transport.requests[1].path == expected_path, (
        "the write must address the path selected by the lookup"
    )
    assert transport.requests[1].payload == _publisher_payload(), (
        "the bounded payload must cross the port unchanged"
    )


def test_publisher_binds_its_publication_operation_to_the_callable_port() -> None:
    """Keep the callable publisher contract the trusted command depends on."""
    module = _publisher_module()
    transport = _RecordingTransport(
        responses=[b'{"total_count":0,"check_runs":[]}', b"{}"]
    )
    publisher = module.GitHubCheckRunPublisher("leynos/netsuke", transport)
    publish: cabc.Callable[[dict[str, object]], None] = publisher.publish

    publish(_publisher_payload())

    assert [request.method for request in transport.requests] == ["GET", "POST"], (
        "the bound publication operation must cross the transport port"
    )


def test_publisher_finds_a_match_beyond_the_first_page(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Follow the server's page total instead of concluding from one page."""
    module = _publisher_module()
    monkeypatch.setattr(module, "CHECK_RUN_PAGE_SIZE", 2)
    first_page = (
        b'{"total_count":3,"check_runs":['
        b'{"id":11,"name":"CodeScene coverage","external_id":"124"},'
        b'{"id":12,"name":"CodeScene coverage","external_id":"125"}]}'
    )
    second_page = (
        b'{"total_count":3,"check_runs":['
        b'{"id":13,"name":"CodeScene coverage","external_id":"123"}]}'
    )
    transport = _RecordingTransport(responses=[first_page, second_page, b"{}"])
    publisher = module.GitHubCheckRunPublisher("leynos/netsuke", transport)

    publisher.publish(_publisher_payload())

    assert [request.method for request in transport.requests] == [
        "GET",
        "GET",
        "PATCH",
    ], "the lookup must read the following page before it publishes"
    assert all("filter=all" in request.path for request in transport.requests[:2]), (
        "the lookup must ask for every Check Run, not only the latest per name"
    )
    assert "page=1" in transport.requests[0].path, "the lookup must start at page 1"
    assert "page=2" in transport.requests[1].path, (
        "the lookup must follow the server's page total"
    )
    assert transport.requests[2].path == "/repos/leynos/netsuke/check-runs/13", (
        "a match on a later page must be updated rather than duplicated"
    )


@contextlib.contextmanager
def _check_run_server(
    responses: list[tuple[int, bytes]],
) -> cabc.Iterator[tuple[str, list[_RequestRecord]]]:
    """Serve fixed Check Run responses and capture the real HTTP protocol."""
    records: list[_RequestRecord] = []

    def capture(handler: http.server.BaseHTTPRequestHandler) -> None:
        """Record one bounded request and emit its configured response."""
        content_length = int(handler.headers.get("Content-Length", "0"))
        records.append(
            _RequestRecord(
                handler.command,
                handler.path,
                dict(handler.headers),
                handler.rfile.read(content_length),
            )
        )
        status, body = responses.pop(0)
        handler.send_response(status)
        handler.send_header("Content-Length", str(len(body)))
        handler.end_headers()
        handler.wfile.write(body)

    class Handler(http.server.BaseHTTPRequestHandler):
        """Capture one bounded local Check Run API request.

        ``BaseHTTPRequestHandler`` dispatches on one ``do_<VERB>`` hook per
        verb, so each hook hands its request to the shared capture.
        """

        def do_GET(self) -> None:
            """Record a GET request and send the next configured response."""
            capture(self)

        def do_POST(self) -> None:
            """Record a POST request and send the next configured response."""
            capture(self)

        def do_PATCH(self) -> None:
            """Record a PATCH request and send the next configured response."""
            capture(self)

        # ruff:ignore[builtin-argument-shadowing] -- inherited stdlib handler signature.
        def log_message(self, format: str, *_args: object) -> None:
            """Suppress local contract-server request logging in test output."""

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


def test_github_publisher_creates_a_check_run_over_real_http() -> None:
    """Use the documented GET-then-POST protocol with bounded API headers."""
    module = _publisher_module()
    with _check_run_server([
        (200, b'{"total_count":0,"check_runs":[]}'),
        (201, b"{}"),
    ]) as server:
        origin, records = server
        transport = module.GitHubApiTransport("token", origin)
        publisher = module.GitHubCheckRunPublisher("leynos/netsuke", transport)
        publisher.publish(_publisher_payload())

    assert [record.method for record in records] == ["GET", "POST"], (
        "a create must look up the idempotency key before posting"
    )
    assert records[0].path.startswith(
        "/repos/leynos/netsuke/commits/" + "a" * 40 + "/check-runs?check_name="
    ), "the lookup must search the originating commit by fixed name"
    assert records[1].path == "/repos/leynos/netsuke/check-runs", (
        "the create must post to the repository Check Run endpoint"
    )
    assert records[1].headers["Authorization"] == "Bearer token", (
        "the step-local token must authorize the request"
    )
    assert json.loads(records[1].body) == _publisher_payload(), (
        "the bounded payload must be published unchanged"
    )


def test_github_publisher_updates_an_existing_idempotency_key() -> None:
    """Patch a matching external ID instead of creating a duplicate Check Run."""
    module = _publisher_module()
    response = (
        b'{"total_count":1,"check_runs":[{"id":17,'
        b'"name":"CodeScene coverage","external_id":"123"}]}'
    )
    with _check_run_server([(200, response), (200, b"{}")]) as server:
        origin, records = server
        transport = module.GitHubApiTransport("token", origin)
        publisher = module.GitHubCheckRunPublisher("leynos/netsuke", transport)
        publisher.publish(_publisher_payload())

    assert [record.method for record in records] == ["GET", "PATCH"], (
        "a matching external ID must be updated rather than recreated"
    )
    assert records[1].path == "/repos/leynos/netsuke/check-runs/17", (
        "the update must address the matched Check Run"
    )


def test_github_publisher_rejects_an_oversized_response() -> None:
    """Bound the Check Run API response before it can consume runner memory."""
    module = _publisher_module()
    with _check_run_server([
        (200, b'{"total_count":0,"check_runs":[]}'),
        (201, b"x" * (module.MAXIMUM_RESPONSE_BYTES + 1)),
    ]) as server:
        origin, _ = server
        transport = module.GitHubApiTransport("token", origin)
        publisher = module.GitHubCheckRunPublisher("leynos/netsuke", transport)
        with pytest.raises(module.CheckRunPublicationError, match="exceeds limit"):
            publisher.publish(_publisher_payload())
