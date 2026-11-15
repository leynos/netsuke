"""Read and verify one downloaded cargo-binstall archive.

This adapter owns the first half of what the consumer smoke does with a
published archive: translating a damaged gzip or tar stream into one typed
refusal, mapping each tar header into the policy's member record, validating
the whole member layout, and checking the archive against its published
``.sha256`` sidecar.

Every safety decision about member names and kinds belongs to
:mod:`release_consumer_archive_policy`; this module only maps tar metadata into
the policy's records and refuses an archive the policy has not accepted.
Writing the binary out is :mod:`release_binstall_extract`, and running it is
:mod:`release_binstall_probe`; the archive is re-scanned there rather than
trusting the layout the caller passed, so a file swapped between the scan and
the copy is refused instead of extracted.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_archive.py``.
"""

import contextlib
import dataclasses
import gzip
import tarfile
import typing as typ
import zlib

import release_consumer_archive_policy as policy
import release_digest
import release_digest_files

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: Largest archive the smoke will read. A cargo-binstall archive for this
#: project is single-digit megabytes, so a larger download describes something
#: other than the release asset.
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024

#: Largest checksum sidecar that will be read. One checksum line is under a
#: hundred bytes; the bound stops a hostile body being read into memory.
MAX_SIDECAR_BYTES = 4096

#: Exceptions a damaged gzip or tar stream raises in this interpreter. Caught
#: together because each describes the same diagnosis: the bytes are not the
#: archive the release declared. Shared with :mod:`release_binstall_extract`,
#: which reads the member data after this module has read the index.
CORRUPT_STREAM = (tarfile.TarError, EOFError, zlib.error, gzip.BadGzipFile)


class ArchiveRefusedError(ValueError):
    """Report a downloaded archive the smoke will not use."""


class ArchiveEmptyError(ArchiveRefusedError):
    """Report an archive file that carries no bytes."""


class ArchiveFileTooLargeError(ArchiveRefusedError):
    """Report an archive file larger than the fixed cap."""


class ArchiveDigestMismatchError(
    ArchiveRefusedError, release_digest.DigestMismatchError
):
    """Report archive bytes that differ from the trusted digest."""


class ArchiveCorruptError(ArchiveRefusedError):
    """Report bytes that do not decompress to a well-formed tar archive."""


class SidecarVerificationError(ArchiveRefusedError, release_digest.DigestError):
    """Report a sidecar that does not describe the downloaded archive."""


class ExtractionError(ArchiveRefusedError):
    """Report a binary that could not be written into the fresh directory."""


@dataclasses.dataclass(frozen=True, slots=True)
class ValidatedArchive:
    """Hold an open archive and the layout its member index was accepted for.

    Parameters
    ----------
    archive
        The open gzip-compressed tar archive, owned by the context manager
        that yielded this value.
    layout
        What the policy decided about every member in the index.
    """

    archive: tarfile.TarFile
    layout: policy.ArchiveLayout


def scan_archive(
    archive_path: pathlib.Path, *, expected_sha256: str
) -> policy.ArchiveLayout:
    """Verify the archive's size and digest, then return its safe layout.

    Returns
    -------
    release_consumer_archive_policy.ArchiveLayout
        The single root binary and the member totals.

    Raises
    ------
    ArchiveEmptyError
        If the file carries no bytes.
    ArchiveFileTooLargeError
        If the file exceeds :data:`MAX_ARCHIVE_BYTES`.
    ArchiveDigestMismatchError
        If the bytes differ from ``expected_sha256``.

    Notes
    -----
    Bytes that decompress to something other than a well-formed tar archive
    fail as :class:`ArchiveCorruptError`, and an unsafe member layout as a
    :class:`release_consumer_archive_policy.ArchiveLayoutError`; both are
    raised while the archive is opened and read.

    Examples
    --------
    >>> import io, tempfile
    >>> from pathlib import Path
    >>> import release_digest_files
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     path = Path(tmp) / "netsuke.tar.gz"
    ...     with tarfile.open(path, "w:gz") as archive:
    ...         entry = tarfile.TarInfo("netsuke")
    ...         entry.size = 1
    ...         _ = archive.addfile(entry, io.BytesIO(b"x"))
    ...     digest = release_digest_files.digest_of_file(path)
    ...     scan_archive(path, expected_sha256=digest).binary_size
    1
    """
    size = archive_path.stat().st_size
    if size == 0:
        msg = f"the archive {archive_path.name} is empty"
        raise ArchiveEmptyError(msg)
    if size > MAX_ARCHIVE_BYTES:
        msg = (
            f"the archive {archive_path.name} is {size} bytes, more than the "
            f"{MAX_ARCHIVE_BYTES} the smoke will read"
        )
        raise ArchiveFileTooLargeError(msg)
    observed = release_digest_files.digest_of_file(archive_path)
    if not release_digest.constant_time_equal(observed, expected_sha256):
        msg = (
            f"the archive {archive_path.name} hashes to {observed}, but the "
            f"trusted record expects {expected_sha256}"
        )
        raise ArchiveDigestMismatchError(msg)
    with validated_archive(archive_path) as validated:
        return validated.layout


@contextlib.contextmanager
def validated_archive(archive_path: pathlib.Path) -> cabc.Iterator[ValidatedArchive]:
    """Open ``archive_path`` and validate its member layout before yielding.

    Yields
    ------
    ValidatedArchive
        The open archive and the layout its member index was accepted for.

    Notes
    -----
    Bytes that are not a readable gzip-compressed tar archive fail as
    :class:`ArchiveCorruptError`, raised while the archive is opened.

    An unsafe member layout is refused by
    :func:`release_consumer_archive_policy.require_safe_layout` before the
    value is yielded, so the caller never holds an archive the policy has not
    accepted.

    Examples
    --------
    >>> import io, tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     path = Path(tmp) / "netsuke.tar.gz"
    ...     with tarfile.open(path, "w:gz") as archive:
    ...         entry = tarfile.TarInfo("netsuke")
    ...         entry.size = 1
    ...         _ = archive.addfile(entry, io.BytesIO(b"x"))
    ...     with validated_archive(path) as validated:
    ...         validated.layout.binary_name
    'netsuke'
    """
    archive = _opened_archive(archive_path)
    try:
        yield ValidatedArchive(archive=archive, layout=_scanned_layout(archive))
    finally:
        archive.close()


def _opened_archive(archive_path: pathlib.Path) -> tarfile.TarFile:
    """Open the downloaded archive, translating a damaged stream.

    Returns
    -------
    tarfile.TarFile
        The open archive. The caller owns it and must close it.

    Raises
    ------
    ArchiveCorruptError
        If the bytes are not a readable gzip-compressed tar archive.
    """
    try:
        return tarfile.open(archive_path, mode="r:gz")
    except CORRUPT_STREAM as error:
        msg = f"the downloaded archive is not a readable gzip tar file: {error}"
        raise ArchiveCorruptError(msg) from error


def _scanned_layout(archive: tarfile.TarFile) -> policy.ArchiveLayout:
    """Return the policy's layout for an open archive's member index.

    Returns
    -------
    release_consumer_archive_policy.ArchiveLayout
        The single root binary and the member totals the policy accepted.

    Raises
    ------
    ArchiveCorruptError
        If the member index cannot be read to its end.
    """
    try:
        members = tuple(_member(entry) for entry in archive.getmembers())
    except CORRUPT_STREAM as error:
        msg = f"the archive's member index is corrupt: {error}"
        raise ArchiveCorruptError(msg) from error
    return policy.require_safe_layout(members)


def _member(entry: tarfile.TarInfo) -> policy.ArchiveMember:
    """Map one tar header into the policy's member record.

    Returns
    -------
    release_consumer_archive_policy.ArchiveMember
        The member name, its kind, and its declared size.

    Examples
    --------
    >>> entry = tarfile.TarInfo("netsuke")
    >>> _member(entry).kind
    <MemberKind.REGULAR: 'regular'>
    """
    return policy.ArchiveMember(
        name=entry.name, kind=_member_kind(entry), size=entry.size
    )


def _member_kind(entry: tarfile.TarInfo) -> policy.MemberKind:
    """Classify one tar header without reading its data.

    Returns
    -------
    release_consumer_archive_policy.MemberKind
        The kind extraction would create for this member.

    Notes
    -----
    A table rather than a chain of branches: the header predicates are
    mutually exclusive, so the first match is the answer, and a kind added to
    the policy fails visibly here instead of silently classifying as
    ``other``.
    """
    recognised = (
        (entry.isreg, policy.MemberKind.REGULAR),
        (entry.isdir, policy.MemberKind.DIRECTORY),
        (entry.issym, policy.MemberKind.SYMLINK),
        (entry.islnk, policy.MemberKind.HARDLINK),
        (entry.isdev, policy.MemberKind.DEVICE),
        (entry.isfifo, policy.MemberKind.FIFO),
    )
    return next(
        (kind for predicate, kind in recognised if predicate()),
        policy.MemberKind.OTHER,
    )


def verify_sidecar(archive_path: pathlib.Path, sidecar_path: pathlib.Path) -> str:
    """Require the published sidecar to describe the downloaded archive.

    Returns
    -------
    str
        The digest the sidecar records, which equals the archive's own.

    Raises
    ------
    SidecarVerificationError
        If the sidecar is empty, unreadable, not UTF-8, malformed, names a
        different asset, or records a different digest.

    Notes
    -----
    The sidecar's *own* digest is checked by the caller against the trusted
    record before this runs, so a substituted sidecar is refused before its
    text is compared with anything.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_files
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     archive = Path(tmp) / "netsuke.tar.gz"
    ...     _ = archive.write_bytes(b"netsuke")
    ...     digest = release_digest_files.digest_of_file(archive)
    ...     sidecar = Path(tmp) / "netsuke.tar.gz.sha256"
    ...     _ = sidecar.write_text(
    ...         digest + "  netsuke.tar.gz" + chr(10), encoding="utf-8"
    ...     )
    ...     verify_sidecar(archive, sidecar) == digest
    True
    """
    text = _read_sidecar_text(sidecar_path)
    actual = release_digest_files.digest_of_file(archive_path)
    try:
        return release_digest.verify_sidecar(archive_path.name, actual, text)
    except release_digest.DigestError as error:
        msg = f"the sidecar does not verify {archive_path.name}: {error}"
        raise SidecarVerificationError(msg) from error


def _read_sidecar_text(sidecar_path: pathlib.Path) -> str:
    """Return the sidecar's text, or refuse one that is not one checksum line.

    Returns
    -------
    str
        The decoded sidecar body.

    Notes
    -----
    Every refusal is :class:`SidecarVerificationError`, raised by the two
    helpers this composes.
    """
    return _decode_sidecar(sidecar_path, _read_sidecar_bytes(sidecar_path))


def _read_sidecar_bytes(sidecar_path: pathlib.Path) -> bytes:
    """Return the sidecar's body, refusing one that is unreadable or not one line.

    Returns
    -------
    bytes
        The body, which is neither empty nor larger than the cap.

    Raises
    ------
    SidecarVerificationError
        If the sidecar cannot be read, is empty, or is larger than
        :data:`MAX_SIDECAR_BYTES`.

    Notes
    -----
    The body is read one byte past the cap so an oversized file is recognised
    from its first read rather than after it has been read whole.
    """
    try:
        with sidecar_path.open("rb") as handle:
            text_bytes = handle.read(MAX_SIDECAR_BYTES + 1)
    except OSError as error:
        msg = f"the sidecar {sidecar_path.name} could not be read: {error}"
        raise SidecarVerificationError(msg) from error
    if not text_bytes:
        msg = f"the sidecar {sidecar_path.name} is empty"
        raise SidecarVerificationError(msg)
    if len(text_bytes) > MAX_SIDECAR_BYTES:
        msg = (
            f"the sidecar {sidecar_path.name} is larger than "
            f"{MAX_SIDECAR_BYTES} bytes, so it is not one checksum line"
        )
        raise SidecarVerificationError(msg)
    return text_bytes


def _decode_sidecar(sidecar_path: pathlib.Path, text_bytes: bytes) -> str:
    """Decode the sidecar's body, refusing bytes that are not UTF-8 text."""
    try:
        return text_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        msg = f"the sidecar {sidecar_path.name} is not UTF-8 text: {error}"
        raise SidecarVerificationError(msg) from error
