"""Exercise one-shot crates.io upload framing and safe diagnostics."""

import http.client
import json
import typing as typ

import crates_io_transport_cli
import pytest
from crates_io_transport_cli import (
    EXIT_OK,
    EXIT_UPLOAD_FAILED,
    UploadFrameError,
    _upload_frame,
    main,
)
from crates_io_transport_support import (
    TEST_CREDENTIAL,
    FakeOpener,
    FakeResponse,
    assert_upload_frame_matches,
    http_error_response,
    upload_args,
)

if typ.TYPE_CHECKING:
    import pathlib


@pytest.mark.parametrize(
    "environment",
    [
        pytest.param({}, id="missing-token"),
        pytest.param({"CRATES_IO_TOKEN": ""}, id="empty-token"),
    ],
)
def test_upload_without_a_usable_token_sends_no_request(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    environment: dict[str, str],
) -> None:
    """Refuse a missing or empty token before contacting the registry."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    _ = metadata.write_text("{}", encoding="utf-8")
    _ = archive.write_bytes(b"bytes")
    opener = FakeOpener(FakeResponse(200, b"{}"))

    result = main(
        upload_args(metadata, archive), opener=opener, environment=environment
    )

    assert result == EXIT_UPLOAD_FAILED, "an unusable token must fail the upload"
    assert not opener.requests, "never send an unauthenticated registry upload"
    assert "no short-lived token" in capsys.readouterr().err, (
        "name the missing credential in the diagnostic"
    )


def test_upload_rejects_metadata_that_is_not_a_json_object(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a metadata array before framing it for the registry."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    _ = metadata.write_text("[]", encoding="utf-8")
    _ = archive.write_bytes(b"bytes")
    opener = FakeOpener(FakeResponse(200, b"{}"))

    result = main(
        upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "non-object metadata must fail the upload"
    assert not opener.requests, "refuse the frame before the one-shot PUT"
    assert "metadata must be a JSON object" in capsys.readouterr().err, (
        "state which part of the frame was invalid"
    )


@pytest.mark.parametrize(
    ("metadata_size", "archive_size", "is_rejected"),
    [
        pytest.param(0, 0, False, id="empty-fields-fit"),
        pytest.param(8, 0, False, id="metadata-at-the-limit"),
        pytest.param(0, 8, False, id="archive-at-the-limit"),
        pytest.param(9, 0, True, id="metadata-past-the-limit"),
        pytest.param(0, 9, True, id="archive-past-the-limit"),
    ],
)
def test_upload_frame_bounds_each_field_at_the_addressable_length(
    monkeypatch: pytest.MonkeyPatch,
    metadata_size: int,
    archive_size: int,
    *,
    is_rejected: bool,
) -> None:
    """Reject either field once it exceeds the unsigned 32-bit length.

    The real limit is 4 GiB minus one, so allocating at it would exhaust the
    machine. The comparison is exercised against a shrunk limit instead: it is
    the same expression, and the boundary cases stay cheap.
    """
    monkeypatch.setattr(crates_io_transport_cli, "MAX_FRAME_LENGTH", 8)

    if is_rejected:
        with pytest.raises(UploadFrameError):
            _upload_frame(bytes(metadata_size), bytes(archive_size))
        return

    frame = _upload_frame(bytes(metadata_size), bytes(archive_size))
    assert len(frame) == 8 + metadata_size + archive_size, (
        "encode both length prefixes without padding"
    )


def test_upload_rejects_an_oversized_frame_before_sending_a_request(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Reject an oversized CLI frame before the opener can send it."""
    monkeypatch.setattr(crates_io_transport_cli, "MAX_FRAME_LENGTH", 8)
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "candidate.crate"
    _ = metadata.write_bytes(b"{}")
    _ = archive.write_bytes(b"123456789")
    opener = FakeOpener(FakeResponse(200, b"{}"))

    result = main(
        upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "an oversized frame must fail the upload"
    assert not opener.requests, "never send a frame that cannot be represented"
    assert "unsigned 32-bit" in capsys.readouterr().err, (
        "diagnose the fixed-width upload limit"
    )


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
        upload_args(metadata, archive),
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
        upload_args(metadata, archive),
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
        upload_args(metadata, archive),
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
        upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "an ambiguous upload response must fail"
    captured = capsys.readouterr()
    assert TEST_CREDENTIAL not in captured.out + captured.err, (
        "redact the short-lived token from diagnostics"
    )
    assert len(opener.requests) == 1, "never retry an ambiguous upload"


def test_upload_treats_an_incomplete_http_response_as_ambiguous(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail once on a truncated upload response without leaking the token."""
    metadata = tmp_path / "metadata.json"
    archive = tmp_path / "archive.crate"
    _ = metadata.write_bytes(b"{}")
    _ = archive.write_bytes(b"candidate")
    opener = FakeOpener(http.client.IncompleteRead(TEST_CREDENTIAL.encode(), 1))

    result = main(
        upload_args(metadata, archive),
        opener=opener,
        environment={"CRATES_IO_TOKEN": TEST_CREDENTIAL},
    )

    assert result == EXIT_UPLOAD_FAILED, "a truncated write response is ambiguous"
    assert len(opener.requests) == 1, "never retry an ambiguous upload"
    captured = capsys.readouterr()
    assert "outcome is ambiguous" in captured.err, "diagnose the uncertain write"
    assert TEST_CREDENTIAL not in captured.out + captured.err, (
        "redact the short-lived token"
    )
