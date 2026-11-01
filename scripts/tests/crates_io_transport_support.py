"""Provide deterministic HTTP responses for registry transport tests."""

import http.client
import io
import struct
import urllib.error
import urllib.request


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
