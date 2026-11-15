"""Read and verify one downloaded cargo-binstall archive.

This adapter owns the first half of what the consumer smoke does with a
published archive: opening the download once, checking its size and digest
through that descriptor, translating a damaged gzip or tar stream into one
typed refusal, mapping each tar header into the policy's member record, and
validating the whole member layout.

Every safety decision about member names and kinds belongs to
:mod:`release_consumer_archive_policy`; this module only maps tar metadata into
the policy's records and refuses an archive the policy has not accepted.
Writing the binary out is :mod:`release_binstall_extract`, running it is
:mod:`release_binstall_probe`, and checking its published ``.sha256`` sidecar
is :mod:`release_binstall_sidecar`. Verification and extraction share one open
archive, so the bytes hashed are the bytes copied; extraction still compares
the layout it was handed with the one its own scan produces, so a caller that
passes a layout from some other archive is refused.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_archive.py``.
"""

import contextlib
import dataclasses
import gzip
import os
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

    Notes
    -----
    Every refusal :func:`verified_archive` can raise passes through here: an
    empty file as :class:`ArchiveEmptyError`, one past
    :data:`MAX_ARCHIVE_BYTES` as :class:`ArchiveFileTooLargeError`, differing
    bytes as :class:`ArchiveDigestMismatchError`, a damaged stream as
    :class:`ArchiveCorruptError`, and an unsafe member layout as a
    :class:`release_consumer_archive_policy.ArchiveLayoutError`.

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
    with verified_archive(archive_path, expected_sha256=expected_sha256) as validated:
        return validated.layout


@contextlib.contextmanager
def verified_archive(
    archive_path: pathlib.Path, *, expected_sha256: str
) -> cabc.Iterator[ValidatedArchive]:
    """Open ``archive_path`` once, verify its bytes, and validate its members.

    Yields
    ------
    ValidatedArchive
        The open archive and the layout its member index was accepted for.

    Notes
    -----
    The digest is computed from the descriptor the members are then read
    through, so the bytes that were hashed are the bytes that will be parsed
    and, for a caller that extracts, copied. Checking a pathname and then
    reopening it would leave a window in which the name resolves to a
    different file whose members still match the accepted layout.

    Bytes that are not a readable gzip-compressed tar archive fail as
    :class:`ArchiveCorruptError`, raised while the archive is opened, and an
    unsafe member layout is refused by
    :func:`release_consumer_archive_policy.require_safe_layout` before the
    value is yielded, so the caller never holds an archive the policy has not
    accepted.

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
    ...     with verified_archive(path, expected_sha256=digest) as validated:
    ...         validated.layout.binary_name
    'netsuke'
    """
    handle = _opened_file(archive_path)
    with handle:
        _require_size(handle, archive_path)
        _require_trusted_digest(handle, archive_path, expected_sha256)
        archive = _opened_archive(handle)
        try:
            yield ValidatedArchive(archive=archive, layout=_scanned_layout(archive))
        finally:
            archive.close()


def _opened_file(archive_path: pathlib.Path) -> typ.IO[bytes]:
    """Open the downloaded archive for reading, translating an I/O failure.

    Returns
    -------
    typing.IO[bytes]
        The open file. The caller owns it and must close it.

    Raises
    ------
    ArchiveRefusedError
        If the file cannot be opened, so a missing or unreadable download is
        reported as a refusal rather than as a bare ``OSError``.
    """
    try:
        return archive_path.open("rb")
    except OSError as error:
        msg = f"the archive {archive_path.name} could not be read: {error}"
        raise ArchiveRefusedError(msg) from error


def _require_size(handle: typ.IO[bytes], archive_path: pathlib.Path) -> None:
    """Require the opened file to carry bytes and to fit the fixed cap.

    Raises
    ------
    ArchiveEmptyError
        If the file carries no bytes.
    ArchiveFileTooLargeError
        If the file exceeds :data:`MAX_ARCHIVE_BYTES`.
    """
    size = os.fstat(handle.fileno()).st_size
    if size == 0:
        msg = f"the archive {archive_path.name} is empty"
        raise ArchiveEmptyError(msg)
    if size > MAX_ARCHIVE_BYTES:
        msg = (
            f"the archive {archive_path.name} is {size} bytes, more than the "
            f"{MAX_ARCHIVE_BYTES} the smoke will read"
        )
        raise ArchiveFileTooLargeError(msg)


def _require_trusted_digest(
    handle: typ.IO[bytes], archive_path: pathlib.Path, expected_sha256: str
) -> None:
    """Require the opened file's bytes to match the trusted digest.

    Raises
    ------
    ArchiveDigestMismatchError
        If the bytes differ from ``expected_sha256``.

    Notes
    -----
    The stream is rewound afterwards, because the same descriptor is handed to
    the tar reader and must start at the archive's first byte.
    """
    observed = release_digest_files.digest_of_stream(handle)
    if not release_digest.constant_time_equal(observed, expected_sha256):
        msg = (
            f"the archive {archive_path.name} hashes to {observed}, but the "
            f"trusted record expects {expected_sha256}"
        )
        raise ArchiveDigestMismatchError(msg)
    handle.seek(0)


def _opened_archive(handle: typ.IO[bytes]) -> tarfile.TarFile:
    """Open the downloaded archive, translating a damaged stream.

    Returns
    -------
    tarfile.TarFile
        The open archive. The caller owns it and must close it.

    Raises
    ------
    ArchiveCorruptError
        If the bytes are not a readable gzip-compressed tar archive.

    Notes
    -----
    The archive is read through ``handle``, whose position must be its first
    byte. Closing the tar reader leaves the descriptor to its owner, so the
    context manager that opened it closes it exactly once.
    """
    try:
        return tarfile.open(fileobj=handle, mode="r:gz")
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
