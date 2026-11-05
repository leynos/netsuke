"""Provide deterministic HTTP responses and CLI arguments for transport tests.

Both transport test modules drive the same command-line surface, so the
argument builders live here rather than beside either module's assertions. A
flag change then reaches every caller at once instead of drifting apart.
"""

import http.client
import io
import struct
import typing as typ
import urllib.error
import urllib.request

if typ.TYPE_CHECKING:
    import pathlib

TEST_CREDENTIAL = "short-lived-test-value"
CRATE_NAME = "netsuke-build"
VERSION = "1.2.3"


def registry_args(operation: str, paths: dict[str, pathlib.Path]) -> list[str]:
    """Build registry identity arguments followed by ordered path options."""
    arguments = [operation, "--crate", CRATE_NAME, "--version", VERSION]
    for option, path in paths.items():
        arguments.extend((option, str(path)))
    return arguments


def probe_args(status_path: pathlib.Path, body_path: pathlib.Path) -> list[str]:
    """Build one registry probe invocation."""
    return registry_args(
        "probe", {"--status-file": status_path, "--body-file": body_path}
    )


def download_args(
    archive_path: pathlib.Path,
    status_path: pathlib.Path,
    report_path: pathlib.Path | None = None,
) -> list[str]:
    """Build one bounded archive-download invocation."""
    paths = {"--archive": archive_path, "--status-file": status_path}
    if report_path is not None:
        paths["--report-file"] = report_path
    return registry_args("download", paths)


def upload_args(metadata_path: pathlib.Path, archive_path: pathlib.Path) -> list[str]:
    """Build one one-shot upload invocation."""
    return [
        "upload",
        "--metadata",
        str(metadata_path),
        "--archive",
        str(archive_path),
    ]


class HandlerListOwner(typ.Protocol):
    """Describe urllib objects that expose their configured handlers."""

    handlers: list[urllib.request.BaseHandler]


class ResponseWithMessage(typ.Protocol):
    """Expose the status phrase attached to urllib redirect responses."""

    msg: str


def unpack_upload_frame(payload: bytes) -> tuple[bytes, bytes, int]:
    """Decode the two little-endian lengths from one captured upload payload."""
    metadata_length = struct.unpack("<I", payload[:4])[0]
    archive_length_offset = metadata_length + 4
    archive_length_end = archive_length_offset + 4
    archive_length = struct.unpack(
        "<I", payload[archive_length_offset:archive_length_end]
    )[0]
    return (
        payload[4:archive_length_offset],
        payload[archive_length_end:],
        archive_length,
    )


def assert_upload_frame_matches(
    payload: bytes, metadata: bytes, archive: bytes
) -> None:
    """Assert the captured frame contains the exact metadata and archive bytes."""
    framed_metadata, framed_archive, archive_length = unpack_upload_frame(payload)
    assert framed_metadata == metadata, "the frame must preserve metadata JSON"
    assert framed_archive == archive, "the frame must preserve candidate bytes"
    assert archive_length == len(archive), "the archive length must match its bytes"


class FakeResponse(io.BytesIO):
    """Act like a bounded ``urllib`` response body."""

    def __init__(self, status: int, body: bytes) -> None:
        """Store one status and body for an opener test."""
        super().__init__(body)
        self.status = status


class FakeOpener:
    """Record requests and return or raise queued transport outcomes."""

    def __init__(self, *outcomes: FakeResponse | BaseException) -> None:
        """Keep the ordered outcomes this opener should produce."""
        self.outcomes = list(outcomes)
        self.requests: list[tuple[urllib.request.Request, float]] = []

    def __call__(
        self, request: urllib.request.Request, *, timeout: float
    ) -> FakeResponse:
        """Record a request and return its next configured outcome."""
        self.requests.append((request, timeout))
        outcome = self.outcomes.pop(0)
        if isinstance(outcome, BaseException):
            raise outcome
        return outcome


def missing_response(url: str, body: bytes = b"missing") -> urllib.error.HTTPError:
    """Build an HTTPError carrying the status and body of an absent response."""
    return http_error_response(url, 404, body)


def http_error_response(url: str, status: int, body: bytes) -> urllib.error.HTTPError:
    """Build an HTTPError for one non-success status and response body."""
    return urllib.error.HTTPError(
        url, status, "HTTP error", http.client.HTTPMessage(), io.BytesIO(body)
    )
