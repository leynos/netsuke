"""Drive bounded crates.io probe, download, and upload commands.

The registry HTTP adapter owns request construction and redirect policy; the
transport HTTP adapter captures responses. This CLI writes observations for
offline policy commands and injects testable external boundaries.

>>> _upload_frame(b"{}", b"archive")[:4] == struct.pack("<I", 2)
True
"""

import argparse
import collections.abc as cabc
import dataclasses
import http.client
import json
import os
import pathlib
import struct
import sys
import time
import urllib.parse

import crates_io_registry_http
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


type Opener = cabc.Callable[..., crates_io_registry_http.HttpResponse]
type Environment = cabc.Mapping[str, str]
type Sleeper = cabc.Callable[[float], None]
type MonotonicClock = cabc.Callable[[], int]


@dataclasses.dataclass(frozen=True, slots=True)
class TransportTiming:
    """Carry the delay and monotonic clock used by bounded downloads."""

    sleeper: Sleeper = time.sleep
    monotonic_clock_ns: MonotonicClock = time.monotonic_ns


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
    """Carry archive inputs, paths, and injected opener, sleeper, and clock."""

    crate: str
    version: str
    archive_path: pathlib.Path
    status_path: pathlib.Path
    opener: Opener
    sleeper: Sleeper
    monotonic_clock_ns: MonotonicClock
    report_path: pathlib.Path | None = None


class UploadFrameError(ValueError):
    """Report metadata or archive bytes that exceed registry framing limits."""

    def __str__(self) -> str:
        """Explain the fixed-width framing limit."""
        return "crates.io upload fields must fit in unsigned 32-bit lengths"


def _probe_url(crate: str, version: str) -> str:
    """Build a URL for one encoded crate version observation."""
    encoded_crate = urllib.parse.quote(crate, safe="-_")
    encoded_version = urllib.parse.quote(version, safe="-._")
    return f"{REGISTRY_API}/crates/{encoded_crate}/{encoded_version}"


def probe(request_data: ProbeRequest) -> int:
    """Record one probe and fail closed without an HTTP status.

    Parameters
    ----------
    request_data : ProbeRequest
        Crate identity, output paths, and opener.

    Returns
    -------
    int
        ``EXIT_OK`` or ``EXIT_TRANSPORT_ERROR``.
    """
    request = crates_io_registry_http.build_request(
        _probe_url(request_data.crate, request_data.version),
        headers=release_crates_io_adapter.required_headers(),
    )
    try:
        status, body = crates_io_transport_http.request_bytes(
            request_data.opener, request, timeout=REQUEST_TIMEOUT_SECONDS
        )
        crates_io_transport_http.save_response(
            request_data.status_path, request_data.body_path, status, body
        )
    except (OSError, http.client.HTTPException) as error:
        print(f"crates.io probe returned no HTTP status: {error}", file=sys.stderr)
        return EXIT_TRANSPORT_ERROR
    return EXIT_OK


def download(request_data: DownloadRequest) -> int:
    """Poll the archive endpoint inside the fixed attempt budget.

    Parameters
    ----------
    request_data : DownloadRequest
        Crate identity, output paths, opener, sleeper, and monotonic clock.

    Returns
    -------
    int
        ``EXIT_OK`` after recording a response or unavailable sentinel.

    Notes
    -----
    If every attempt lacks a response, write status ``0`` and empty bytes for
    the offline verifier to reject. A deterministic transport failure, such as
    a rejected redirect target, ends the budget after its first attempt.
    """
    request = crates_io_registry_http.build_request(
        release_crates_io_adapter.download_url(
            request_data.crate, request_data.version
        ),
        headers=release_crates_io_adapter.required_headers(),
    )
    result = crates_io_transport_http.DownloadPollResult(
        response=None, attempts=0, status_class="unknown", last_error=None
    )
    started_ns = request_data.monotonic_clock_ns()
    try:
        result = crates_io_transport_http.poll_archive(
            request_data.opener,
            request,
            crates_io_transport_http.DownloadPollPolicy(
                attempts=DOWNLOAD_ATTEMPTS,
                delay_seconds=DOWNLOAD_DELAY_SECONDS,
                timeout_seconds=REQUEST_TIMEOUT_SECONDS,
            ),
            request_data.sleeper,
        )
        if result.last_error is not None and result.response is None:
            print(
                f"crates.io archive download remained unavailable: {result.last_error}",
                file=sys.stderr,
            )
        if result.response is None:
            crates_io_transport_http.save_response(
                request_data.status_path, request_data.archive_path, 0, b""
            )
        else:
            crates_io_transport_http.save_response(
                request_data.status_path, request_data.archive_path, *result.response
            )
    finally:
        if request_data.report_path is not None:
            elapsed_ms = max(
                0,
                (request_data.monotonic_clock_ns() - started_ns) // 1_000_000,
            )
            crates_io_transport_http.save_download_report(
                request_data.report_path,
                attempts=result.attempts,
                status_class=result.status_class,
                elapsed_ms=elapsed_ms,
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
    """Upload retained bytes once and report registry warnings.

    Parameters
    ----------
    metadata_path, archive_path : pathlib.Path
        Derived registry metadata and retained archive.
    token : str
        Short-lived Trusted Publishing token.
    opener : Opener
        Injected registry opener.

    Returns
    -------
    int
        ``EXIT_OK`` if accepted; otherwise ``EXIT_UPLOAD_FAILED``.
    """
    if not token:
        print("crates.io upload has no short-lived token", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    payload = _upload_payload(metadata_path, archive_path)
    if payload is None:
        return EXIT_UPLOAD_FAILED
    request = crates_io_registry_http.build_request(
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
        status, body = crates_io_transport_http.request_bytes(
            opener, request, timeout=REQUEST_TIMEOUT_SECONDS
        )
    except (OSError, http.client.HTTPException) as error:
        diagnostic = str(error).replace(token, "[REDACTED]")
        print(f"crates.io upload outcome is ambiguous: {diagnostic}", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    accepted, result = release_crates_io_adapter.upload_response(status, body)
    if not accepted:
        diagnostic = str(result).replace(token, "[REDACTED]")
        print(f"crates.io rejected the upload: {diagnostic}", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    if result not in (None, [], {}):
        warning_text = json.dumps(result, sort_keys=True).replace(token, "[REDACTED]")
        print(f"crates.io upload warnings: {warning_text}")
    return EXIT_OK


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for the transport commands.

    Returns
    -------
    argparse.ArgumentParser
        Parser for the probe, bounded download, and upload commands.
    """
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
    archive_download.add_argument("--report-file", type=pathlib.Path)
    registry_upload = commands.add_parser("upload", help="publish retained crate bytes")
    registry_upload.add_argument("--metadata", required=True, type=pathlib.Path)
    registry_upload.add_argument("--archive", required=True, type=pathlib.Path)
    return parser


def main(
    argv: list[str] | None = None,
    *,
    opener: Opener | None = None,
    environment: Environment | None = None,
    timing: TransportTiming | None = None,
) -> int:
    """Run one transport command using injected external boundaries.

    Parameters
    ----------
    argv : list[str] or None
        Command arguments; use process arguments when omitted.
    opener : Opener or None
        Registry opener; use the allowlisted opener when omitted.
    environment : Environment or None
        Environment mapping used to read the upload token.
    timing : TransportTiming
        Delay and monotonic clock for bounded archive polling.

    Returns
    -------
    int
        ``EXIT_OK``, ``EXIT_TRANSPORT_ERROR``, ``EXIT_UPLOAD_FAILED``, or
        ``EXIT_USAGE``.
    """
    try:
        arguments = build_parser().parse_args(argv)
    except SystemExit as error:
        return EXIT_OK if error.code == 0 else EXIT_USAGE
    selected_opener = opener or crates_io_registry_http.default_opener
    selected_timing = TransportTiming() if timing is None else timing
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
                crate=arguments.crate,
                version=arguments.version,
                archive_path=arguments.archive,
                status_path=arguments.status_file,
                opener=selected_opener,
                sleeper=selected_timing.sleeper,
                monotonic_clock_ns=selected_timing.monotonic_clock_ns,
                report_path=arguments.report_file,
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
