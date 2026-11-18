"""Read and verify one downloaded cargo-binstall archive.

This adapter owns the first half of what the consumer smoke does with a
published archive: opening the download once, checking its size and digest
through that descriptor, translating a damaged gzip or tar stream into one
typed refusal, and reading the member index through to its end.

Every safety decision about member names and kinds belongs to
:mod:`release_consumer_archive_policy`; mapping tar headers into the policy's
records is :mod:`release_binstall_members`, and this module refuses an archive
the policy has not accepted.
Writing the binary out is :mod:`release_binstall_extract`, running it is
:mod:`release_binstall_probe`, and checking its ``.sha256`` sidecar is
:mod:`release_binstall_sidecar`. Verification and extraction share one open
archive, so the bytes hashed are the bytes copied; extraction still compares the
layout it was handed with the one its own scan produces.

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

import release_binstall_inflate as inflate
import release_binstall_members as members
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

#: What a refused parse translates into :class:`ArchiveCorruptError`.
#:
#: ``tarfile`` validates a header's checksum before it parses the fields, so
#: most damage arrives as a :class:`tarfile.TarError`. A field that does parse
#: as a checksum but not as what it claims to be escapes that gate: the GNU
#: sparse metadata is read outside the header parser's own guard, and a
#: malformed one raises a bare ``ValueError``. Listing it here reports such an
#: archive as refused rather than as a traceback.
#:
#: It is kept apart from :data:`CORRUPT_STREAM` because
#: :class:`ArchiveRefusedError` is a ``ValueError``: every site that catches
#: this tuple must let a refusal through unchanged, and extraction, which
#: shares :data:`CORRUPT_STREAM`, must not translate an inflation refusal into
#: corruption.
PARSE_FAULTS = (*CORRUPT_STREAM, ValueError)


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


class ArchiveInflationError(ArchiveRefusedError):
    """Report an archive that expands past the decompressed-byte cap."""


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
    headers
        Every member header, in archive order, as the scan read them.
        Iterating a tar archive consumes its index, so a later
        ``getmembers()`` would return nothing; retaining the headers is what
        lets extraction find the binary without re-reading the index.
    layout
        What the policy decided about every member in the index.
    """

    archive: tarfile.TarFile
    headers: tuple[tarfile.TarInfo, ...]
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
    accepted. The stream is read to its end before the value is yielded, so a
    download whose gzip trailer is missing or wrong is refused here too.
    """
    handle = _opened_file(archive_path)
    with handle:
        _require_size(handle, archive_path)
        _require_trusted_digest(handle, archive_path, expected_sha256)
        archive, stream = _opened_archive(handle)
        try:
            headers = _scanned_headers(archive)
            layout = _scanned_layout(headers)
            _require_index_tail(stream)
            yield ValidatedArchive(archive=archive, headers=headers, layout=layout)
        finally:
            archive.close()
            stream.close()


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


def _opened_archive(
    handle: typ.IO[bytes],
) -> tuple[tarfile.TarFile, inflate.InflatedStream]:
    """Open the downloaded archive over a bounded decompressing stream.

    Returns
    -------
    tuple[tarfile.TarFile, release_binstall_inflate.InflatedStream]
        The open archive and the stream it reads through. The caller owns both
        and must close them.

    Raises
    ------
    ArchiveCorruptError
        If the bytes are not a readable gzip-compressed tar archive, including
        a header whose fields the parser cannot read.
    ArchiveRefusedError
        If the stream refuses the bytes before the parser sees them, such as
        when the first header exceeds the budget; it passes through unchanged.

    Notes
    -----
    The archive is read through ``handle``, whose position must be its first
    byte, and every decompressed byte is counted against
    :data:`release_binstall_inflate.MAX_INFLATED_BYTES` before ``tarfile``
    parses it. Closing the tar reader leaves both ``handle`` and the stream to
    their owners, so each is closed exactly once.
    """
    stream = inflate.InflatedStream(
        handle, limit=inflate.MAX_INFLATED_BYTES, refusal=ArchiveInflationError
    )
    try:
        return tarfile.open(fileobj=stream, mode="r:"), stream
    except ArchiveRefusedError:
        stream.close()
        raise
    except PARSE_FAULTS as error:
        stream.close()
        msg = f"the downloaded archive is not a readable gzip tar file: {error}"
        raise ArchiveCorruptError(msg) from error


def _require_index_tail(stream: inflate.InflatedStream) -> None:
    """Read past the archive's end marker, requiring nothing but zero bytes.

    Raises
    ------
    ArchiveCorruptError
        If the compressed stream is damaged after the archive's end marker, or
        carries a non-zero byte there.
    ArchiveRefusedError
        If the stream refuses a read past the budget; it passes through.

    Notes
    -----
    ``tarfile`` stops at the first all-zero block it reads, so anything after
    that block — a second archive appended to the first, or a header whose
    checksum is wrong — is never parsed and so never refused. Reading the
    decompressed stream to its end is what sees it. Standard writers pad the
    end marker with further zero blocks, so zero bytes are accepted; any other
    byte means these are not the bytes the index described.

    Reading to the end is also what lets gzip verify the trailer it recorded:
    the end marker leaves the decompressor short of it, and closing the reader
    performs no check, so a download missing its trailer would otherwise be
    accepted.
    """
    try:
        while block := stream.read(inflate.CHUNK_BYTES):
            if block.strip(b"\x00"):
                msg = "the archive carries data after its end marker"
                raise ArchiveCorruptError(msg)
    except ArchiveRefusedError:
        raise
    except PARSE_FAULTS as error:
        msg = f"the archive's compressed stream is corrupt: {error}"
        raise ArchiveCorruptError(msg) from error


def _scanned_headers(archive: tarfile.TarFile) -> tuple[tarfile.TarInfo, ...]:
    """Return the archive's member headers, refusing an over-long index.

    Returns
    -------
    tuple[tarfile.TarInfo, ...]
        Every header the walk read, in archive order: the whole index, or one
        past the cap when the index is longer than that.

    Raises
    ------
    ArchiveCorruptError
        If the member index cannot be read to its end, including a header whose
        fields the parser cannot read.
    ArchiveRefusedError
        If the stream refuses a header the budget does not admit. The stream's
        own refusal passes through unchanged.

    Notes
    -----
    The headers are walked one at a time rather than through ``getmembers()``,
    which reads the whole index into memory before anything can refuse it. The
    walk stops one header past the policy's member cap, so an over-long index
    costs the cap plus one header rather than every header the archive claims;
    :func:`_scanned_layout` then refuses those headers as
    :class:`release_consumer_archive_policy.ArchiveTooLargeError`. The headers
    are kept because iterating a tar archive consumes its index.
    """
    headers: list[tarfile.TarInfo] = []
    try:
        for entry in archive:
            headers.append(entry)
            if len(headers) > policy.MAX_MEMBERS:
                break
    except ArchiveRefusedError:
        raise
    except PARSE_FAULTS as error:
        msg = f"the archive's member index is corrupt: {error}"
        raise ArchiveCorruptError(msg) from error
    return tuple(headers)


def _scanned_layout(
    headers: cabc.Sequence[tarfile.TarInfo],
) -> policy.ArchiveLayout:
    """Return the policy's layout for headers an archive declared.

    Returns
    -------
    release_consumer_archive_policy.ArchiveLayout
        The single root binary and the member totals the policy accepted.
    """
    return members.require_layout(headers)
