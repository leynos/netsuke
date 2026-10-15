"""Behavioural coverage for published-bytes verification.

After publication the release downloads the archive back from the CDN and
checks it against the digest the identity record holds. The check is only
worth having if absence and mismatch are distinguished: reporting a checksum
mismatch for a crate that was simply never published sends a maintainer
looking for corrupted bytes.

The absence signature is measured, not assumed. ``static.crates.io`` refuses a
missing key with **403 and a ~263-byte S3 ``AccessDenied`` XML body** — not an
empty response. A verifier testing only for an empty file would hash that XML
and report a mismatch.

Run via ``make test-release-publication``.
"""

import hashlib
import typing as typ

import pytest
import release_digest
from verify_crates_io_bytes import (
    ABSENT_STATUSES,
    ArchiveAbsentError,
    VerificationError,
    classify_download,
    declared_verdicts,
    download_url,
    plan_verification,
    required_headers,
    verify_downloaded_archive,
    verify_status_is_readable,
)

if typ.TYPE_CHECKING:
    import pathlib

#: gzip magic, as decimal integers. Spelled without escape sequences on
#: purpose: an escape inside a non-raw docstring or literal is evaluated when
#: Python builds the string, and ``\\x1f`` in a docstring becomes a real
#: control byte that the doctest compiler then refuses.
GZIP_MAGIC = bytes([31, 139, 8, 0])

#: The measured shape of an absent key's refusal. Only the opening tag is
#: significant to the classifier, so the fixture mirrors the real body's head
#: and enough following bytes to exceed any plausible prefix test.
ACCESS_DENIED_BODY = (
    b'<?xml version="1.0" encoding="UTF-8"?><Error><Code>AccessDenied</Code>'
    b"<Message>Access Denied</Message><RequestId>REQ</RequestId></Error>"
)

ARCHIVE_BYTES = GZIP_MAGIC + b"netsuke crate payload"
INTENDED = hashlib.sha256(ARCHIVE_BYTES).hexdigest()


@pytest.fixture
def archive(tmp_path: pathlib.Path) -> pathlib.Path:
    """Write the archive's bytes to a file, as a completed download would."""
    path = tmp_path / "netsuke-build-0.1.0-beta4.crate"
    path.write_bytes(ARCHIVE_BYTES)
    return path


def test_download_url_names_the_crate_and_version() -> None:
    """Require the URL to address the crate the release published."""
    url = download_url("netsuke-build", "0.1.0-beta4")
    assert url == (
        "https://static.crates.io/crates/netsuke-build/netsuke-build-0.1.0-beta4.crate"
    ), "the URL must name the crate and the version being downloaded"


@pytest.mark.parametrize(
    ("status", "verdict"),
    [
        (200, "present"),
        (403, "absent"),
        (404, "absent"),
        (500, "error"),
        (503, "error"),
    ],
)
def test_classify_download_separates_absence_from_failure(
    status: int, verdict: str
) -> None:
    """Require absence and failure to be distinct verdicts.

    Both hosts answer 404 for an absent version and the CDN answers 403 for an
    absent key, so a non-200 status alone cannot mean "not published".
    """
    assert classify_download(status) == verdict, (
        "absence and failure must be distinct verdicts"
    )


def test_classify_download_agrees_with_the_absent_set() -> None:
    """Require the classification to use the declared absent statuses."""
    for status in ABSENT_STATUSES:
        assert classify_download(status) == "absent", (
            "every declared absent status must classify as absent"
        )


def test_verify_accepts_the_intended_bytes(archive: pathlib.Path) -> None:
    """Require downloaded bytes matching the record to verify."""
    assert (
        verify_downloaded_archive(
            archive,
            crate="netsuke-build",
            version="0.1.0-beta4",
            expected_sha256=INTENDED,
        )
        == INTENDED
    ), "the downloaded bytes must verify to the recorded digest"


def test_verify_rejects_bytes_that_differ(archive: pathlib.Path) -> None:
    """Require different bytes to be catchable under both hierarchies.

    A mismatch is reported by this module and by nothing else, so that
    ``except VerificationError`` catches it. It is *also* a
    ``release_digest.DigestMismatchError``, because that is what it is: the
    digest comparison is owned by that module, and a caller already guarding
    it must not need a second clause for the published-bytes path.
    """
    with pytest.raises(VerificationError) as excinfo:
        verify_downloaded_archive(
            archive,
            crate="netsuke-build",
            version="0.1.0-beta4",
            expected_sha256="0" * 64,
        )
    assert "netsuke-build" in str(excinfo.value), (
        "the diagnostic must name the crate whose bytes differ"
    )
    assert isinstance(excinfo.value, release_digest.DigestMismatchError), (
        "a mismatch must be catchable as a digest mismatch"
    )
    assert isinstance(excinfo.value, release_digest.DigestError), (
        "a mismatch must be catchable as a digest error"
    )


def test_every_failure_is_catchable_as_verification_error(
    archive: pathlib.Path, tmp_path: pathlib.Path
) -> None:
    """Require one ``except`` clause to cover every failure path.

    Both an absent archive and a mismatched one are raised here. If either
    escaped ``VerificationError``, the workflow's single guard would report an
    unhandled traceback instead of a diagnosis.
    """
    absent = tmp_path / "absent.crate"
    absent.write_bytes(b"")
    with pytest.raises(VerificationError):
        verify_downloaded_archive(
            absent, crate="x", version="1", expected_sha256=INTENDED
        )
    with pytest.raises(VerificationError):
        verify_downloaded_archive(
            archive, crate="x", version="1", expected_sha256="0" * 64
        )


def test_verify_refuses_an_empty_archive(tmp_path: pathlib.Path) -> None:
    """Require zero bytes to be reported as absent, not as a mismatch."""
    empty = tmp_path / "empty.crate"
    empty.write_bytes(b"")
    with pytest.raises(ArchiveAbsentError):
        verify_downloaded_archive(
            empty, crate="x", version="1", expected_sha256=INTENDED
        )


def test_verify_refuses_an_access_denied_body_as_absent(
    tmp_path: pathlib.Path,
) -> None:
    """Require the CDN's refusal document to read as absent, not as corruption.

    This is the case the measured behaviour forces. The body is neither empty
    nor the archive; hashing it would produce a mismatch, which reports a
    content problem where the real answer is "not published".
    """
    refused = tmp_path / "netsuke.crate"
    refused.write_bytes(ACCESS_DENIED_BODY)
    assert len(ACCESS_DENIED_BODY) > 0, "fixture must not be empty"
    with pytest.raises(ArchiveAbsentError):
        verify_downloaded_archive(
            refused,
            crate="netsuke-build",
            version="0.1.0-beta4",
            expected_sha256=INTENDED,
        )


def test_a_real_archive_is_not_mistaken_for_a_refusal(archive: pathlib.Path) -> None:
    """Require a genuine gzip archive to pass the absence test.

    The absence test keys on an XML prefix. A gzip stream begins with
    ``0x1f 0x8b``, so the two cannot collide — asserted here so a later
    widening of the prefix test cannot quietly start rejecting real archives.
    """
    assert archive.read_bytes()[:2] == GZIP_MAGIC[:2], (
        "the genuine archive must still carry the gzip magic"
    )
    assert not archive.read_bytes()[:1].startswith(b"<"), (
        "the genuine archive must not carry the refusal document's XML prefix"
    )
    assert (
        verify_downloaded_archive(
            archive,
            crate="netsuke-build",
            version="0.1.0-beta4",
            expected_sha256=INTENDED,
        )
        == INTENDED
    ), "a real archive must pass the absence test and verify"


def test_status_readability_refuses_an_error_status() -> None:
    """Require a server error to raise rather than proceed to a digest check."""
    with pytest.raises(VerificationError):
        verify_status_is_readable(500, crate="netsuke-build", version="0.1.0-beta4")


def test_required_headers_names_a_user_agent() -> None:
    """Require an explicit ``User-Agent``.

    Measured: ``crates.io/api`` answers a request with no ``User-Agent`` with
    **403 and a zero-byte body**. Without this header the API calls fail
    closed with no diagnosable reason, and the 403 is indistinguishable from an
    absent version.
    """
    headers = required_headers()
    lowered = {key.lower(): value for key, value in headers.items()}
    assert "user-agent" in lowered, "the required headers must carry a User-Agent entry"
    assert lowered["user-agent"].strip(), "the User-Agent value must not be blank"


@pytest.mark.parametrize(
    ("status", "expected_sha256", "actual_sha256", "verdict"),
    [
        (200, INTENDED, INTENDED, "verified"),
        (200, INTENDED, "0" * 64, "mismatch"),
        (403, INTENDED, None, "absent"),
        (404, INTENDED, None, "absent"),
        (500, INTENDED, None, "error"),
    ],
)
def test_plan_verification_maps_every_observable_case(
    status: int, expected_sha256: str, actual_sha256: str | None, verdict: str
) -> None:
    """Require the verdict to follow from status and digests alone."""
    assert (
        plan_verification(
            status=status, expected_sha256=expected_sha256, actual_sha256=actual_sha256
        )
        == verdict
    ), "the verdict must follow from the status and the two digests"


def test_the_declared_verdicts_are_all_reachable() -> None:
    """Require every declared verdict to be produced by some input.

    A verdict named in ``declared_verdicts`` but unreachable from
    ``plan_verification`` would let a workflow branch on a case that never
    occurs, which reads as covered behaviour.
    """
    reachable = {
        plan_verification(status=s, expected_sha256=INTENDED, actual_sha256=a)
        for s in (200, 403, 500)
        for a in (INTENDED, "0" * 64, None)
    }
    assert set(declared_verdicts()) <= reachable, (
        "every declared verdict must be produced by some input"
    )
