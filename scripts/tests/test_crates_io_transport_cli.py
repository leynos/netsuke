"""Specify bounded crates.io probe, upload, and downloaded-byte transport."""

import io
import json
import typing as typ
import urllib.error
import urllib.request
import urllib.response
from email.message import Message

import crates_io_transport_http
import pytest
from crates_io_transport_cli import (
    DOWNLOAD_ATTEMPTS,
    EXIT_OK,
    EXIT_UPLOAD_FAILED,
    main,
)
from crates_io_transport_http import RegistryUrlError
from crates_io_transport_support import (
    FakeOpener,
    FakeResponse,
    HandlerListOwner,
    ResponseWithMessage,
    assert_upload_frame_matches,
    http_error_response,
    missing_response,
)

if typ.TYPE_CHECKING:
    import pathlib

TEST_CREDENTIAL = "short-lived-test-value"
CRATE_NAME = "netsuke-build"
VERSION = "1.2.3"
REGISTRY_PREFIX = "https://crates.io/api/v1/crates/"


def test_build_request_rejects_an_unallowlisted_initial_url() -> None:
    """Reject a non-registry destination at the public request builder."""
    with pytest.raises(RegistryUrlError):
        crates_io_transport_http.build_request(
            "https://attacker.example/forged-response", headers={}
        )


def test_default_opener_rejects_an_unallowlisted_redirect_target(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Apply the installed request handler to a real urllib redirect chain."""
    opener = typ.cast("HandlerListOwner", crates_io_transport_http._HTTP_OPENER)
    https_handler = next(
        handler
        for handler in opener.handlers
        if isinstance(handler, urllib.request.HTTPSHandler)
    )

    def redirect_response(
        request: urllib.request.Request,
    ) -> urllib.response.addinfourl:
        """Return a redirect response aimed at an unallowlisted host.

        Parameters
        ----------
        request : urllib.request.Request
            Request whose URL becomes the response origin.

        Returns
        -------
        urllib.response.addinfourl
            A redirect to the attacker-controlled test host.

        Examples
        --------
        >>> redirect_response(urllib.request.Request("https://crates.io")).code
        302
        """
        headers = Message()
        headers["Location"] = "https://attacker.example/forged-response"
        response = urllib.response.addinfourl(
            io.BytesIO(), headers, request.full_url, code=302
        )
        typ.cast("ResponseWithMessage", response).msg = "Found"
        return response

    monkeypatch.setattr(https_handler, "https_open", redirect_response)
    request = crates_io_transport_http.build_request(
        f"{REGISTRY_PREFIX}{CRATE_NAME}/{VERSION}", headers={}
    )

    with pytest.raises(RegistryUrlError):
        crates_io_transport_http.default_opener(request, timeout=1.0)


def _registry_args(operation: str, paths: dict[str, pathlib.Path]) -> list[str]:
    """Build registry identity arguments followed by ordered path options."""
    arguments = [
        operation,
        "--crate",
        CRATE_NAME,
        "--version",
        VERSION,
    ]
    for option, path in paths.items():
        arguments.extend((option, str(path)))
    return arguments


def _probe_args(status_path: pathlib.Path, body_path: pathlib.Path) -> list[str]:
    """Build one reusable registry probe invocation."""
    return _registry_args(
        "probe", {"--status-file": status_path, "--body-file": body_path}
    )


def _download_args(
    archive_path: pathlib.Path,
    status_path: pathlib.Path,
    report_path: pathlib.Path | None = None,
) -> list[str]:
    """Build one reusable archive-download invocation."""
    paths = {"--archive": archive_path, "--status-file": status_path}
    if report_path is not None:
        paths["--report-file"] = report_path
    return _registry_args("download", paths)


def _upload_args(metadata_path: pathlib.Path, archive_path: pathlib.Path) -> list[str]:
    """Build one reusable one-shot upload invocation."""
    return [
        "upload",
        "--metadata",
        str(metadata_path),
        "--archive",
        str(archive_path),
    ]


def test_probe_saves_http_status_and_body_without_retry(
    tmp_path: pathlib.Path,
) -> None:
    """Record a response once, including an HTTPError that carries a status."""
    status = tmp_path / "status.txt"
    body = tmp_path / "body.json"
    opener = FakeOpener(
        missing_response(
            "https://crates.io/api/v1/crates/netsuke-build/1.2.3",
            b'{"errors":[]}',
        )
    )

    result = main(_probe_args(status, body), opener=opener)

    assert result == EXIT_OK, "a recorded HTTP response should complete the probe"
    assert len(opener.requests) == 1, "the registry probe must not retry"
    assert status.read_text(encoding="ascii").strip() == "404", (
        "save the registry's HTTP status"
    )
    assert body.read_bytes() == b'{"errors":[]}', "save the response body unchanged"


def test_probe_uses_the_adapter_user_agent_and_bounded_timeout(
    tmp_path: pathlib.Path,
) -> None:
    """Keep registry identity and the request timeout at the adapter boundary."""
    opener = FakeOpener(FakeResponse(200, b'{"version":{}}'))
    result = main(_probe_args(tmp_path / "status", tmp_path / "body"), opener=opener)

    assert result == EXIT_OK, "a successful probe should record its response"
    request, timeout = opener.requests[0]
    user_agent = request.get_header("User-agent")
    assert user_agent is not None, "set the registry client User-Agent"
    assert user_agent.startswith("netsuke"), (
        "identify the registry client with the required User-Agent"
    )
    assert timeout > 0, "bound the request with a positive timeout"


def test_download_retries_transient_responses_then_stops_after_success(
    tmp_path: pathlib.Path,
) -> None:
    """Retry a 5xx response but never repeat a successful archive download."""
    delays: list[float] = []
    opener = FakeOpener(
        http_error_response(
            "https://static.crates.io/crates/netsuke-build/netsuke-build-1.2.3.crate",
            503,
            b"busy",
        ),
        FakeResponse(200, b"exact registry archive"),
    )
    archive = tmp_path / "candidate.crate"
    status = tmp_path / "status.txt"
    report = tmp_path / "download-report.json"

    result = main(
        _download_args(archive, status, report),
        opener=opener,
        sleeper=delays.append,
    )

    assert result == EXIT_OK, "the download step records its final response"
    assert len(opener.requests) == 2, "retry the transient response once before success"
    assert delays == [10.0], "wait the configured delay between retries"
    assert archive.read_bytes() == b"exact registry archive", (
        "retain the downloaded registry bytes unchanged"
    )
    assert status.read_text(encoding="ascii").strip() == "200", (
        "record the successful download status"
    )
    diagnostics = json.loads(report.read_text(encoding="ascii"))
    assert diagnostics["attempts"] == 2, "record the retry count"
    assert diagnostics["status_class"] == "2xx", "record only the status class"
    assert diagnostics["elapsed_ms"] >= 0, "record elapsed time"


def test_download_retries_absence_only_to_its_fixed_attempt_budget(
    tmp_path: pathlib.Path,
) -> None:
    """Keep eventual archive propagation inside a finite polling budget."""
    download_url = (
        "https://static.crates.io/crates/netsuke-build/netsuke-build-1.2.3.crate"
    )
    opener = FakeOpener(*[
        http_error_response(download_url, 404, b"absent")
        for _ in range(DOWNLOAD_ATTEMPTS)
    ])
    delays: list[float] = []
    archive = tmp_path / "candidate.crate"
    status = tmp_path / "status.txt"

    result = main(
        _download_args(archive, status),
        opener=opener,
        sleeper=delays.append,
    )

    assert result == EXIT_OK, "the transport records exhausted polling for verification"
    assert len(opener.requests) == DOWNLOAD_ATTEMPTS, "stop at the fixed attempt budget"
    assert len(delays) == DOWNLOAD_ATTEMPTS - 1, "do not delay after the final attempt"
    assert status.read_text(encoding="ascii").strip() == "404", (
        "retain the final absence status"
    )
    assert archive.read_bytes() == b"absent", "retain the final response body"


def test_download_records_no_status_after_transient_network_exhaustion(
    tmp_path: pathlib.Path,
) -> None:
    """Save an indeterminate sentinel so the offline verifier fails closed."""
    failures = [urllib.error.URLError("offline") for _ in range(DOWNLOAD_ATTEMPTS)]
    opener = FakeOpener(*failures)
    archive = tmp_path / "candidate.crate"
    status = tmp_path / "status.txt"

    result = main(
        _download_args(archive, status),
        opener=opener,
        sleeper=lambda _delay: None,
    )

    assert result == EXIT_OK, "the offline verifier consumes the transport sentinel"
    assert status.read_text(encoding="ascii").strip() == "0", (
        "record missing HTTP status as indeterminate"
    )
    assert archive.read_bytes() == b"", "do not invent a response body"


def test_upload_frames_metadata_and_exact_archive_bytes(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Send Cargo's documented two-length frame and surface registry warnings."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    metadata_bytes = b'{"name":"netsuke-build","vers":"1.2.3"}'
    archive_bytes = b"retained crate bytes"
    _ = metadata.write_bytes(metadata_bytes)
    _ = archive.write_bytes(archive_bytes)
    opener = FakeOpener(FakeResponse(200, b'{"warnings":{"other":["accepted"]}}'))

    result = main(
        _upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_OK, "a valid crates.io upload response should succeed"
    request, _timeout = opener.requests[0]
    payload = request.data
    assert isinstance(payload, bytes), "frame upload data as exact binary bytes"
    assert_upload_frame_matches(payload, metadata_bytes, archive_bytes)
    assert request.get_header("Authorization") == f"Bearer {TEST_CREDENTIAL}", (
        "send the exchanged token with the registry's Bearer scheme"
    )
    assert request.get_header("Accept") == "application/json", (
        "request a JSON upload response"
    )
    assert request.get_header("Content-type") == "application/octet-stream", (
        "send the registry's binary upload framing"
    )
    assert request.get_method() == "PUT", "publish through the registry upload method"
    assert request.full_url == "https://crates.io/api/v1/crates/new", (
        "publish to the crates.io upload endpoint"
    )
    assert TEST_CREDENTIAL not in capsys.readouterr().out, "never print the token"


def test_upload_fails_on_registry_errors_and_never_retries(
    tmp_path: pathlib.Path,
) -> None:
    """Treat a 2xx error body as failure and do not repeat an ambiguous upload."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    _ = metadata.write_text(json.dumps({"name": "netsuke-build"}), encoding="utf-8")
    _ = archive.write_bytes(b"bytes")
    opener = FakeOpener(FakeResponse(201, b'{"errors":[{"detail":"rejected"}]}'))

    result = main(
        _upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "a response containing errors is a failure"
    assert len(opener.requests) == 1, "never retry an upload in the same job"


@pytest.mark.parametrize(
    ("body", "expected_diagnostic"),
    [
        pytest.param(
            b'{"errors":[{"detail":"metadata rejected"}]}',
            "HTTP status 422: metadata rejected",
            id="include-registry-detail",
        ),
        pytest.param(
            f'{{"errors":[{{"detail":"{TEST_CREDENTIAL}"}}]}}'.encode(),
            "HTTP status 422: [REDACTED]",
            id="redact-token-in-registry-detail",
        ),
        pytest.param(b"not JSON", "HTTP status 422", id="malformed-body-fallback"),
        pytest.param(
            b'{"message":"rejected"}',
            "HTTP status 422",
            id="missing-detail-fallback",
        ),
    ],
)
def test_upload_reports_registry_error_details_safely(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    body: bytes,
    expected_diagnostic: str,
) -> None:
    """Show usable registry errors and fall back without exposing the token."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    _ = metadata.write_text("{}", encoding="utf-8")
    _ = archive.write_bytes(b"bytes")
    error = http_error_response("https://crates.io/api/v1/crates/new", 422, body)

    result = main(
        _upload_args(metadata, archive),
        opener=FakeOpener(error),
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "a non-success upload response must fail"
    captured = capsys.readouterr()
    assert expected_diagnostic in captured.err, (
        "diagnose a registry rejection while retaining a status fallback"
    )
    assert TEST_CREDENTIAL not in captured.out + captured.err, (
        "never expose the exchanged token in a registry error detail"
    )


def test_upload_redacts_token_from_ambiguous_network_error(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Do not expose the short-lived token when the request outcome is unknown."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    _ = metadata.write_text("{}", encoding="utf-8")
    _ = archive.write_bytes(b"bytes")
    opener = FakeOpener(OSError(f"connection closed after receiving {TEST_CREDENTIAL}"))

    result = main(
        _upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "an ambiguous upload response must fail"
    captured = capsys.readouterr()
    assert TEST_CREDENTIAL not in captured.out + captured.err, (
        "redact the short-lived token from diagnostics"
    )
    assert len(opener.requests) == 1, "never retry an ambiguous upload"
