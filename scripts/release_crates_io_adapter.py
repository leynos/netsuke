"""Translate crates.io download responses and files into domain observations.

This adapter owns the public download URL, required request headers, HTTP
status mapping, absence-body signature, and file hashing. The verifier receives
only :class:`verify_crates_io_bytes.DownloadObservation`; it never reads a path
or interprets transport values.

Run through ``publish_crates_io_cli``; behavioural coverage lives in
``scripts/tests/test_release_crates_io_adapter.py``.
"""

import typing as typ

import release_digest_files
from verify_crates_io_bytes import DownloadAvailability, DownloadObservation

if typ.TYPE_CHECKING:
    import pathlib

#: Public host serving published crate archives without a token.
DOWNLOAD_HOST = "https://static.crates.io"

#: Statuses the download host returns for an absent key.
ABSENT_STATUSES = frozenset({403, 404})

#: Status the download host returns with archive bytes.
SUCCESS_STATUS = 200

#: Prefix of the XML refusal body observed for a missing archive key.
S3_ERROR_PREFIX = b"<?xml"


def download_url(crate: str, version: str) -> str:
    """Return the token-free URL for one published crate archive.

    Returns
    -------
    str
        The public static.crates.io download URL.

    Examples
    --------
    >>> download_url("netsuke-build", "0.1.0-beta4")
    'https://static.crates.io/crates/netsuke-build/netsuke-build-0.1.0-beta4.crate'
    """
    return f"{DOWNLOAD_HOST}/crates/{crate}/{crate}-{version}.crate"


def required_headers() -> dict[str, str]:
    """Return the request headers required by the public crates.io endpoint.

    Returns
    -------
    dict[str, str]
        Headers identifying the release verifier to the public endpoint.

    Examples
    --------
    >>> required_headers()["User-Agent"].startswith("netsuke")
    True
    """
    return {
        "User-Agent": "netsuke-release-verifier (https://github.com/leynos/netsuke)"
    }


def observe_download_file(status: int, archive: pathlib.Path) -> DownloadObservation:
    """Map an HTTP response and saved body into a domain observation.

    Returns
    -------
    DownloadObservation
        The observed presence, absence, or uncertainty and its digest when
        bytes were present.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     absent = Path(tmp) / "absent.crate"
    ...     _ = absent.write_bytes(b"<?xml version='1.0'?>")
    ...     observe_download_file(200, absent).availability
    <DownloadAvailability.ABSENT: 'absent'>
    >>> observe_download_file(503, Path("missing.crate")).availability
    <DownloadAvailability.INDETERMINATE: 'indeterminate'>
    """
    if status in ABSENT_STATUSES:
        return DownloadObservation(DownloadAvailability.ABSENT)
    if status != SUCCESS_STATUS:
        return DownloadObservation(DownloadAvailability.INDETERMINATE)
    with archive.open("rb") as body:
        prefix = body.read(len(S3_ERROR_PREFIX))
    if not prefix or prefix == S3_ERROR_PREFIX:
        return DownloadObservation(DownloadAvailability.ABSENT)
    digest = release_digest_files.digest_of_file(archive)
    return DownloadObservation(DownloadAvailability.PRESENT, digest)
