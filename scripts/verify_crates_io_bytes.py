"""Fetch a published ``.crate`` and prove its bytes are the ones intended.

Trusted Publishing removes the long-lived token but adds a question the token
never asked: did the bytes the registry now serves come from the artefacts this
run built and verified? The registry answers with a checksum, and the release's
identity record holds the digest it expects. This module fetches the published
archive through the token-free public download path and compares them.

The path is deliberately the *consumer's*. ``static.crates.io`` is served by
Amazon S3 with an immutable, year-long cache policy, so it answers ``403`` for
anything absent -- not ``404``. An earlier draft that treated 404 as "nothing
there" and 403 as an error would report a healthy new crate as broken on the
one run that published it. Both statuses are therefore read as absence, and
the distinction the module draws is between *absent* and *wrong*, never between
two flavours of absent.

Nothing here uploads, deletes, or retries a publication. It reads public bytes
and exits non-zero when they are not the intended ones.

Used by the release workflow's post-publication verification step; behavioural
coverage lives in ``scripts/tests/test_verify_crates_io_bytes.py``.
"""

import pathlib
import typing as typ

import release_digest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Public host serving published crate archives without a token.
DOWNLOAD_HOST = "https://static.crates.io"

#: Status S3 returns for a key it does not hold.
ABSENT_STATUSES = frozenset({403, 404})

#: Status of a key that is present and readable.
OK_STATUS = 200

#: Opening bytes of the XML error document S3 returns for a refused key.
#:
#: ``static.crates.io`` answers an absent key with ``403`` and an
#: ``AccessDenied`` document around 260 bytes long -- not an empty body.
S3_ERROR_PREFIX = b"<?xml"


class VerificationError(RuntimeError):
    """Report that a published archive could not be verified.

    Every failure this module reports derives from here, so one ``except``
    clause covers the whole verification.
    """


class ArchiveAbsentError(VerificationError):
    """Report that the registry serves no archive under the expected key."""


class ArchiveMismatchError(VerificationError, release_digest.DigestMismatchError):
    """Report that published bytes differ from the identity record.

    Derived from two bases deliberately. The comparison is performed by
    :mod:`release_digest`, and the disagreement *is* a
    :class:`release_digest.DigestMismatchError`; naming it here too keeps that
    identity, so a caller guarding either hierarchy catches it without a
    second clause.
    """


def download_url(crate: str, version: str) -> str:
    """Return the public download URL for a published crate version.

    Parameters
    ----------
    crate
        Crate name, such as ``netsuke-build``.
    version
        Published version, such as ``0.1.0-beta4``.

    Returns
    -------
    str
        The token-free ``static.crates.io`` URL for the ``.crate`` archive.

    Notes
    -----
    The key is ``<crate>/<crate>-<version>.crate`` under ``/crates/``. It is
    spelled out here rather than fetched from the API because the API's own
    ``dl_path`` points at ``crates.io``, which redirects to this host; going
    straight to the key keeps the check to one request and keeps the URL
    visible in the workflow log. It still needs the headers
    :func:`required_headers` returns.

    Examples
    --------
    >>> download_url("netsuke-build", "0.1.0-beta4")
    'https://static.crates.io/crates/netsuke-build/netsuke-build-0.1.0-beta4.crate'
    """
    return f"{DOWNLOAD_HOST}/crates/{crate}/{crate}-{version}.crate"


def classify_download(status: int) -> str:
    """Classify a download response as ``"present"`` or ``"absent"``.

    Parameters
    ----------
    status
        HTTP status the download host answered with.

    Returns
    -------
    str
        ``"present"``, ``"absent"``, or ``"error"``.

    Notes
    -----
    ``403`` is read as absence alongside ``404`` because that is what S3
    answers for a key it does not hold. Any other status is ``"error"``: a
    ``500`` or a ``429`` says nothing about whether the archive exists, and
    reporting either as absence would turn a transient outage into a release
    that "verified" a crate it never fetched.

    Examples
    --------
    >>> classify_download(200)
    'present'
    >>> classify_download(403)
    'absent'
    >>> classify_download(404)
    'absent'
    >>> classify_download(503)
    'error'
    """
    if status == OK_STATUS:
        return "present"
    if status in ABSENT_STATUSES:
        return "absent"
    return "error"


def _is_absence_body(archive: pathlib.Path) -> bool:
    """Report whether a downloaded file is an absence response, not an archive.

    Parameters
    ----------
    archive
        Path to a file a download step wrote.

    Returns
    -------
    bool
        ``True`` when the file is empty, or begins with the XML error
        document the download host returns for a key it does not hold.

    Notes
    -----
    Checking the body shape rather than only the size is the difference
    between two diagnoses a maintainer reads differently. A draft testing for
    an empty file would have hashed the error document and reported a checksum
    mismatch, pointing at corrupted bytes when the crate is simply absent.

    Only ``<?xml`` is matched, and only at the start. A ``.crate`` is a gzip
    stream, so it begins with ``\x1f\x8b`` and can never be mistaken for one.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     empty = Path(tmp) / "empty.crate"
    ...     _ = empty.write_bytes(b"")
    ...     refused = Path(tmp) / "refused.crate"
    ...     _ = refused.write_bytes(b'<?xml version="1.0"?><Error/>')
    ...     real = Path(tmp) / "real.crate"
    ...     _ = real.write_bytes(bytes([31, 139, 8, 0]))
    ...     (_is_absence_body(empty), _is_absence_body(refused),
    ...      _is_absence_body(real))
    (True, True, False)
    """
    head = archive.read_bytes()[: len(S3_ERROR_PREFIX)]
    return not head or head == S3_ERROR_PREFIX


def verify_downloaded_archive(
    archive: pathlib.Path, *, crate: str, version: str, expected_sha256: str
) -> str:
    """Require downloaded bytes to be the archive this release intended.

    Parameters
    ----------
    archive
        Path to the downloaded ``.crate`` file.
    crate
        Crate name, used for the diagnosis only.
    version
        Version, used for the diagnosis only.
    expected_sha256
        Bare lowercase hexadecimal digest from the identity record.

    Returns
    -------
    str
        The verified digest, equal to ``expected_sha256``.

    Raises
    ------
    ArchiveAbsentError
        If the downloaded file is empty or holds the host's refusal document,
        which is what an absence response leaves behind.
    ArchiveMismatchError
        If the file's digest is not ``expected_sha256``.

    Notes
    -----
    An empty file is reported as absence rather than as a digest mismatch. A
    403 from the download host, saved to disk, leaves a zero-byte file whose
    digest is a perfectly valid value for nothing at all; calling that a
    mismatch would send a maintainer looking for corrupted bytes instead of a
    missing crate.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     archive = Path(tmp) / "netsuke-build-0.1.0-beta4.crate"
    ...     _ = archive.write_bytes(b"netsuke")
    ...     verify_downloaded_archive(
    ...         archive,
    ...         crate="netsuke-build",
    ...         version="0.1.0-beta4",
    ...         expected_sha256=release_digest.digest_of_file(archive),
    ...     ) == release_digest.digest_of_file(archive)
    True
    """
    if _is_absence_body(archive):
        msg = (
            f"{crate} {version} was not downloaded: {archive.name} holds no "
            f"archive, only what the download host returned for a key it does "
            f"not hold"
        )
        raise ArchiveAbsentError(msg)
    actual = release_digest.digest_of_file(archive)
    if not release_digest.constant_time_equal(actual, expected_sha256):
        msg = (
            f"{crate} {version} published as {actual}, but the identity "
            f"record expects {expected_sha256}"
        )
        raise ArchiveMismatchError(msg)
    return actual


def verify_status_is_readable(status: int, *, crate: str, version: str) -> None:
    """Refuse a download status that disproves nothing but proves nothing.

    Parameters
    ----------
    status
        HTTP status the download host answered with.
    crate
        Crate name, for the diagnosis.
    version
        Version, for the diagnosis.

    Raises
    ------
    ArchiveAbsentError
        If the status means the key is not held.
    VerificationError
        If the status is neither present nor absent.

    Examples
    --------
    >>> verify_status_is_readable(200, crate="c", version="1")
    >>> verify_status_is_readable(403, crate="c", version="1")
    Traceback (most recent call last):
        ...
    verify_crates_io_bytes.ArchiveAbsentError: c 1 is not published: the \
download host holds no such key
    """
    verdict = classify_download(status)
    if verdict == "present":
        return
    if verdict == "absent":
        msg = f"{crate} {version} is not published: the download host holds no such key"
        raise ArchiveAbsentError(msg)
    msg = (
        f"the download host answered {status} for {crate} {version}, which "
        f"neither confirms nor denies the archive; a transient failure must "
        f"not be recorded as a verification result"
    )
    raise VerificationError(msg)


def required_headers() -> dict[str, str]:
    """Return the request headers the public download path requires.

    Returns
    -------
    dict[str, str]
        A descriptive ``User-Agent``.

    Notes
    -----
    ``crates.io`` refuses a request without one, answering ``403`` with a
    zero-byte body -- the same status S3 uses for a missing key, which is why
    this is returned as an explicit value for callers to apply rather than
    left to a default.

    Examples
    --------
    >>> required_headers()["User-Agent"].startswith("netsuke")
    True
    """
    return {
        "User-Agent": "netsuke-release-verifier (https://github.com/leynos/netsuke)"
    }


def plan_verification(
    *, status: int, expected_sha256: str, actual_sha256: str | None
) -> str:
    """Return ``"verified"``, ``"absent"``, or ``"mismatch"`` for a download.

    Parameters
    ----------
    status
        HTTP status the download host answered with.
    expected_sha256
        Digest the identity record holds.
    actual_sha256
        Digest of the downloaded bytes, or ``None`` when none were read.

    Returns
    -------
    str
        The verdict.

    Notes
    -----
    Composed from the same predicates the downloading path uses, so a caller
    that already has a status and a digest can reach the same verdict without
    repeating the ordering. Absence is judged first: a zero-byte body from an
    absent key has a valid digest of its own, and comparing digests first would
    report the one case that is not a mismatch as one.

    Examples
    --------
    >>> plan_verification(status=403, expected_sha256="aa", actual_sha256=None)
    'absent'
    >>> plan_verification(status=200, expected_sha256="aa", actual_sha256="aa")
    'verified'
    >>> plan_verification(status=200, expected_sha256="aa", actual_sha256="bb")
    'mismatch'
    >>> plan_verification(status=429, expected_sha256="aa", actual_sha256=None)
    'error'
    """
    verdict = classify_download(status)
    if verdict == "absent":
        return "absent"
    if verdict == "error":
        return "error"
    if actual_sha256 is None:
        return "absent"
    if release_digest.constant_time_equal(actual_sha256, expected_sha256):
        return "verified"
    return "mismatch"


def declared_verdicts() -> cabc.Mapping[str, str]:
    """Return every verdict this module can reach, for a contract test to pin.

    Returns
    -------
    dict[str, str]
        Verdict name to a one-line description.

    Notes
    -----
    Exposed so a contract test can assert the set is exactly the four handled
    outcomes. A verdict added to :func:`plan_verification` without a row here
    is one no test enumerates, and the caller's dispatch would meet it with no
    branch.

    Examples
    --------
    >>> sorted(declared_verdicts())
    ['absent', 'error', 'mismatch', 'verified']
    """
    return {
        "verified": "the published bytes match the identity record",
        "absent": "the download host holds no archive under the expected key",
        "mismatch": "the published bytes differ from the identity record",
        "error": "the host answered neither present nor absent",
    }
