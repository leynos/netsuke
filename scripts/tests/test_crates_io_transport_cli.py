"""Specify bounded crates.io probe, upload, and downloaded-byte transport."""

import json
import typing as typ
import urllib.error
import urllib.request

import pytest
from crates_io_transport_cli import (
    DOWNLOAD_ATTEMPTS,
    EXIT_OK,
    EXIT_TRANSPORT_ERROR,
    EXIT_UPLOAD_FAILED,
    main,
)
from crates_io_transport_http import RegistryUrlError, _AllowlistedRequestHandler
from crates_io_transport_support import (
    FakeOpener,
    FakeResponse,
    assert_upload_frame_matches,
    http_error_response,
    missing_response,
)

if typ.TYPE_CHECKING:
    import pathlib

TEST_CREDENTIAL = "short-lived-test-value"
CRATE_NAME = "netsuke-build"
VERSION = "1.2.3"


def _probe_args(status_path: pathlib.Path, body_path: pathlib.Path) -> list[str]:
    """Build one reusable registry probe invocation."""
    return [
        "probe",
        "--crate",
        CRATE_NAME,
        "--version",
        VERSION,
        "--status-file",
        str(status_path),
        "--body-file",
        str(body_path),
    ]


def _download_args(archive_path: pathlib.Path, status_path: pathlib.Path) -> list[str]:
    """Build one reusable archive-download invocation."""
    return [
        "download",
        "--crate",
        CRATE_NAME,
        "--version",
        VERSION,
        "--archive",
        str(archive_path),
        "--status-file",
        str(status_path),
    ]


def _upload_args(metadata_path: pathlib.Path, archive_path: pathlib.Path) -> list[str]:
    """Build one reusable one-shot upload invocation."""
    return [
        "upload",
        "--metadata",
        str(metadata_path),
        "--archive",
        str(archive_path),
    ]


@pytest.mark.parametrize(
    ("url", "hook"),
    [
        pytest.param(
            "https://attacker.example/forged-response",
            "https_request",
            id="unallowlisted-host",
        ),
        pytest.param("ftp://crates.io/forged-response", "ftp_request", id="ftp"),
    ],
)
def test_request_handler_rejects_an_unallowlisted_redirect_target(
    url: str, hook: str
) -> None:
    """Recheck the URL urllib opens after following a registry redirect."""
    handler = _AllowlistedRequestHandler(frozenset({"crates.io"}))
    request = urllib.request.Request(  # ruff: ignore[suspicious-url-open-usage] - test a rejected redirect target
        url
    )

    with pytest.raises(RegistryUrlError):
        getattr(handler, hook)(request)


def test_probe_maps_a_rejected_redirect_to_a_transport_failure(
    tmp_path: pathlib.Path,
) -> None:
    """Treat rejected redirect targets as an unavailable registry observation."""
    opener = FakeOpener(RegistryUrlError())

    result = main(_probe_args(tmp_path / "status", tmp_path / "body"), opener=opener)

    assert result == EXIT_TRANSPORT_ERROR, "reject a forged registry probe"
    assert len(opener.requests) == 1, "do not retry a rejected probe redirect"


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


def test_probe_fails_closed_when_no_http_status_exists(
    tmp_path: pathlib.Path,
) -> None:
    """Do not turn a connection failure into an absent registry version."""
    status = tmp_path / "status.txt"
    body = tmp_path / "body.json"
    opener = FakeOpener(urllib.error.URLError("offline"))

    result = main(_probe_args(status, body), opener=opener)

    assert result == EXIT_TRANSPORT_ERROR, "a probe without status must fail closed"
    assert len(opener.requests) == 1, "the failed probe must not retry"
    assert not status.exists(), "a missing status must not be written as HTTP 404"


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

    result = main(
        _download_args(archive, status),
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
