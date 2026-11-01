"""Exchange bounded HTTP requests with crates.io and static.crates.io.

The policy CLI remains offline. This module owns only request construction,
response capture, finite download polling and the registry's upload framing.
Injected opener, environment and sleeper seams keep transport decisions
testable without network access or process-wide environment mutation.

This example checks that the client accepts only the fixed HTTPS hosts:

>>> crates_io_transport_http.is_registry_url("https://crates.io/api/v1")
True
>>> crates_io_transport_http.is_registry_url("file:///tmp/crate")
False
"""

import argparse
import collections.abc as cabc
import dataclasses
import json
import os
import pathlib
import struct
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

import crates_io_transport_http
import release_crates_io_adapter

EXIT_OK = 0
EXIT_TRANSPORT_ERROR = 1
EXIT_UPLOAD_FAILED = 2
EXIT_USAGE = 64

REQUEST_TIMEOUT_SECONDS = 30.0
DOWNLOAD_ATTEMPTS = 12
DOWNLOAD_DELAY_SECONDS = 10.0
MAX_FRAME_LENGTH = (1 << 32) - 1
REGISTRY_API = "https://crates.io/api/v1"
HTTP_SUCCESS_MIN = 200
HTTP_REDIRECT_MIN = 300
HTTP_SERVER_ERROR_MIN = 500
HTTP_STATUS_LIMIT = 600


type Opener = cabc.Callable[..., crates_io_transport_http.HttpResponse]
type Environment = cabc.Mapping[str, str]
type Sleeper = cabc.Callable[[float], None]


@dataclasses.dataclass(frozen=True, slots=True)
class ProbeRequest:
    """Carry registry observation inputs and output paths."""

    crate: str
    version: str
    status_path: pathlib.Path
    body_path: pathlib.Path
    opener: Opener


@dataclasses.dataclass(frozen=True, slots=True)
class DownloadRequest:
    """Carry archive polling inputs, output paths and injected boundaries."""

    crate: str
    version: str
    archive_path: pathlib.Path
    status_path: pathlib.Path
    opener: Opener
    sleeper: Sleeper


class UploadFrameError(ValueError):
    """Report metadata or archive bytes that exceed registry framing limits."""

    def __str__(self) -> str:
        """Explain the fixed-width framing limit."""
        return "crates.io upload fields must fit in unsigned 32-bit lengths"


def _request_bytes(
    opener: Opener, request: urllib.request.Request
) -> tuple[int, bytes]:
    """Return the HTTP status and body, including an HTTPError response."""
    try:
        response = opener(request, timeout=REQUEST_TIMEOUT_SECONDS)
    except urllib.error.HTTPError as error:
        try:
            return error.code, error.read()
        finally:
            error.close()
    try:
        return int(response.status), response.read()
    finally:
        response.close()


def _save_response(
    status_path: pathlib.Path, body_path: pathlib.Path, status: int, body: bytes
) -> None:
    """Write one response body and its status for the offline policy CLI."""
    body_path.write_bytes(body)
    status_path.write_text(f"{status}\n", encoding="ascii")


def _probe_url(crate: str, version: str) -> str:
    """Build a URL for one encoded crate version observation."""
    encoded_crate = urllib.parse.quote(crate, safe="-_")
    encoded_version = urllib.parse.quote(version, safe="-._")
    return f"{REGISTRY_API}/crates/{encoded_crate}/{encoded_version}"


def probe(request_data: ProbeRequest) -> int:
    """Record a single registry probe and fail closed without an HTTP status."""
    request = crates_io_transport_http.build_request(
        _probe_url(request_data.crate, request_data.version),
        headers=release_crates_io_adapter.required_headers(),
    )
    try:
        status, body = _request_bytes(request_data.opener, request)
        _save_response(request_data.status_path, request_data.body_path, status, body)
    except OSError as error:
        print(f"crates.io probe returned no HTTP status: {error}", file=sys.stderr)
        return EXIT_TRANSPORT_ERROR
    return EXIT_OK


def _retryable_download_status(status: int) -> bool:
    """Identify only absent-key and server-error statuses for polling."""
    return (
        status in release_crates_io_adapter.ABSENT_STATUSES
        or HTTP_SERVER_ERROR_MIN <= status < HTTP_STATUS_LIMIT
    )


def download(request_data: DownloadRequest) -> int:
    """Poll the public archive endpoint inside the fixed attempt budget."""
    request = crates_io_transport_http.build_request(
        release_crates_io_adapter.download_url(
            request_data.crate, request_data.version
        ),
        headers=release_crates_io_adapter.required_headers(),
    )
    last_response: tuple[int, bytes] | None = None
    for attempt in range(DOWNLOAD_ATTEMPTS):
        try:
            last_response = _request_bytes(request_data.opener, request)
        except OSError as error:
            if attempt + 1 == DOWNLOAD_ATTEMPTS:
                _save_response(
                    request_data.status_path, request_data.archive_path, 0, b""
                )
                print(
                    f"crates.io archive download remained unavailable: {error}",
                    file=sys.stderr,
                )
                return EXIT_OK
            request_data.sleeper(DOWNLOAD_DELAY_SECONDS)
            continue

        status, body = last_response
        if not _retryable_download_status(status):
            _save_response(
                request_data.status_path, request_data.archive_path, status, body
            )
            return EXIT_OK
        if attempt + 1 < DOWNLOAD_ATTEMPTS:
            request_data.sleeper(DOWNLOAD_DELAY_SECONDS)

    if last_response is None:
        _save_response(request_data.status_path, request_data.archive_path, 0, b"")
    else:
        _save_response(
            request_data.status_path, request_data.archive_path, *last_response
        )
    return EXIT_OK


def _upload_frame(metadata: bytes, archive: bytes) -> bytes:
    """Encode the registry's little-endian metadata and archive lengths."""
    if len(metadata) > MAX_FRAME_LENGTH or len(archive) > MAX_FRAME_LENGTH:
        raise UploadFrameError
    return (
        struct.pack("<I", len(metadata))
        + metadata
        + struct.pack("<I", len(archive))
        + archive
    )


def _upload_response(status: int, body: bytes) -> tuple[bool, object]:
    """Return whether the response is accepted and any registry warnings."""
    if not HTTP_SUCCESS_MIN <= status < HTTP_REDIRECT_MIN:
        detail = _upload_error_detail(body)
        message = f"HTTP status {status}"
        return False, f"{message}: {detail}" if detail is not None else message
    try:
        response = json.loads(body)
    except UnicodeError, json.JSONDecodeError:
        return False, "response body is not valid JSON"
    if not isinstance(response, dict):
        return False, "response body is not a JSON object"
    errors = response.get("errors", [])
    if errors not in (None, []):
        return False, errors
    return True, response.get("warnings")


def _upload_error_detail(body: bytes) -> str | None:
    """Extract registry validation details from an HTTP error body."""
    try:
        response = json.loads(body)
    except UnicodeError, json.JSONDecodeError:
        return None
    if not isinstance(response, dict) or not isinstance(response.get("errors"), list):
        return None
    details = [
        error["detail"]
        for error in response["errors"]
        if isinstance(error, dict) and isinstance(error.get("detail"), str)
    ]
    return "; ".join(details) if details else None


def _upload_payload(
    metadata_path: pathlib.Path, archive_path: pathlib.Path
) -> bytes | None:
    """Read and validate the one-shot upload frame, reporting invalid inputs."""
    try:
        metadata = metadata_path.read_bytes()
        metadata_record = json.loads(metadata)
        archive = archive_path.read_bytes()
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        print(f"crates.io upload input is invalid: {error}", file=sys.stderr)
        return None
    if not isinstance(metadata_record, dict):
        print(
            "crates.io upload input is invalid: metadata must be a JSON object",
            file=sys.stderr,
        )
        return None
    try:
        return _upload_frame(metadata, archive)
    except UploadFrameError as error:
        print(f"crates.io upload input is invalid: {error}", file=sys.stderr)
        return None


def upload(
    *,
    metadata_path: pathlib.Path,
    archive_path: pathlib.Path,
    token: str,
    opener: Opener,
) -> int:
    """Upload exact retained bytes once and report response warnings."""
    if not token:
        print("crates.io upload has no short-lived token", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    payload = _upload_payload(metadata_path, archive_path)
    if payload is None:
        return EXIT_UPLOAD_FAILED
    request = crates_io_transport_http.build_request(
        f"{REGISTRY_API}/crates/new",
        data=payload,
        headers={
            **release_crates_io_adapter.required_headers(),
            "Accept": "application/json",
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/octet-stream",
        },
        method="PUT",
    )
    try:
        status, body = _request_bytes(opener, request)
    except OSError as error:
        diagnostic = str(error).replace(token, "[REDACTED]")
        print(f"crates.io upload outcome is ambiguous: {diagnostic}", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    accepted, result = _upload_response(status, body)
    if not accepted:
        diagnostic = str(result).replace(token, "[REDACTED]")
        print(f"crates.io rejected the upload: {diagnostic}", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    if result not in (None, [], {}):
        warning_text = json.dumps(result, sort_keys=True).replace(token, "[REDACTED]")
        print(f"crates.io upload warnings: {warning_text}")
    return EXIT_OK


def build_parser() -> argparse.ArgumentParser:
    """Build the one-shot probe, bounded download and exact-byte upload CLI."""
    parser = argparse.ArgumentParser(prog="crates_io_transport_cli")
    commands = parser.add_subparsers(dest="command", required=True)
    registry_probe = commands.add_parser("probe", help="record one registry response")
    registry_probe.add_argument("--crate", required=True)
    registry_probe.add_argument("--version", required=True)
    registry_probe.add_argument("--status-file", required=True, type=pathlib.Path)
    registry_probe.add_argument("--body-file", required=True, type=pathlib.Path)
    archive_download = commands.add_parser("download", help="poll for registry bytes")
    archive_download.add_argument("--crate", required=True)
    archive_download.add_argument("--version", required=True)
    archive_download.add_argument("--archive", required=True, type=pathlib.Path)
    archive_download.add_argument("--status-file", required=True, type=pathlib.Path)
    registry_upload = commands.add_parser("upload", help="publish retained crate bytes")
    registry_upload.add_argument("--metadata", required=True, type=pathlib.Path)
    registry_upload.add_argument("--archive", required=True, type=pathlib.Path)
    return parser


def main(
    argv: list[str] | None = None,
    *,
    opener: Opener | None = None,
    environment: Environment | None = None,
    sleeper: Sleeper = time.sleep,
) -> int:
    """Run one transport command using injectable external boundaries."""
    try:
        arguments = build_parser().parse_args(argv)
    except SystemExit as error:
        return EXIT_OK if error.code == 0 else EXIT_USAGE
    selected_opener = opener or crates_io_transport_http.default_opener
    if arguments.command == "probe":
        return probe(
            ProbeRequest(
                arguments.crate,
                arguments.version,
                arguments.status_file,
                arguments.body_file,
                selected_opener,
            )
        )
    if arguments.command == "download":
        return download(
            DownloadRequest(
                arguments.crate,
                arguments.version,
                arguments.archive,
                arguments.status_file,
                selected_opener,
                sleeper,
            )
        )
    selected_environment = os.environ if environment is None else environment
    return upload(
        metadata_path=arguments.metadata,
        archive_path=arguments.archive,
        token=selected_environment.get("CRATES_IO_TOKEN", ""),
        opener=selected_opener,
    )


if __name__ == "__main__":
    raise SystemExit(main())
