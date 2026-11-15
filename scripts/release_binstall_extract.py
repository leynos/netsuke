"""Write the single root binary of a published archive into a fresh directory.

The smoke must run exactly one file: the ``netsuke`` the archive ships. This
adapter performs the only write in the smoke, and it is deliberately narrow.
The archive's bytes are checked against the trusted digest and its member
layout is re-scanned before anything is written, so a file swapped since the
caller's scan is refused rather than extracted. The digest is the part that
matters: a layout records the binary's name and declared size and the member
count, so a replacement archive whose members match would pass a layout-only
check. The output path is created with ``O_CREAT | O_EXCL | O_NOFOLLOW``, so a
directory that already holds a symlink cannot redirect the write, and the copy
is streamed under the declared size so a member that lies about its length is
stopped part-way. A failure deletes the partial output, because a half-written
executable is something a later step could observe and run.

Member safety itself is decided in
:mod:`release_consumer_archive_policy`; this module never writes a member the
policy has not accepted, and it writes no member but the binary.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_extract.py``.

Examples
--------
Extraction writes the one root binary and names it, so the caller runs exactly
the file the archive shipped:

>>> import io, tarfile, tempfile
>>> from pathlib import Path
>>> import release_binstall_archive as archive_adapter
>>> import release_digest_files
>>> with tempfile.TemporaryDirectory() as tmp:
...     root = Path(tmp)
...     path = root / "netsuke.tar.gz"
...     with tarfile.open(path, "w:gz") as archive:
...         entry = tarfile.TarInfo("netsuke")
...         entry.size = 1
...         _ = archive.addfile(entry, io.BytesIO(b"x"))
...     digest = release_digest_files.digest_of_file(path)
...     layout = archive_adapter.scan_archive(path, expected_sha256=digest)
...     out = root / "out"
...     out.mkdir()
...     extract_binary(path, layout, out, expected_sha256=digest).read_bytes()
b'x'
"""

import contextlib
import os
import typing as typ

import release_consumer_archive_policy as policy
import release_digest
import release_digest_files
from release_binstall_archive import (
    CORRUPT_STREAM,
    ArchiveCorruptError,
    ExtractionError,
    validated_archive,
)

if typ.TYPE_CHECKING:
    import pathlib
    import tarfile

#: Block size for copying the binary out of the archive.
COPY_CHUNK_BYTES = 1024 * 1024

#: Permissions the extracted binary carries.
BINARY_MODE = 0o755


def extract_binary(
    archive_path: pathlib.Path,
    layout: policy.ArchiveLayout,
    destination: pathlib.Path,
    *,
    expected_sha256: str,
) -> pathlib.Path:
    """Write the root binary into ``destination`` and return its path.

    Parameters
    ----------
    archive_path
        The verified archive.
    layout
        The layout an earlier scan accepted. The archive is re-scanned, and a
        difference is refused rather than trusted.
    destination
        A caller-created, empty directory.
    expected_sha256
        The trusted digest the archive was verified against. It is the only
        value here that binds extraction to the exact bytes a consumer
        receives, because ``layout`` describes members rather than content.

    Returns
    -------
    pathlib.Path
        The absolute path of the written binary.

    Raises
    ------
    ExtractionError
        If ``destination`` is not an empty directory, the output path is
        already occupied, the binary cannot be written, the bytes written
        differ from the declared size, the archive no longer hashes to
        ``expected_sha256``, or the re-scan disagrees with ``layout``.

    Notes
    -----
    A member whose data cannot be read fails as
    :class:`release_binstall_archive.ArchiveCorruptError`, and an archive that
    no longer passes the member rules as a
    :class:`release_consumer_archive_policy.ArchiveLayoutError`; both are
    raised while the archive is reopened. The module docstring shows a complete
    extraction of a one-member archive.
    """
    _require_fresh_directory(destination)
    _require_trusted_bytes(archive_path, expected_sha256)
    with validated_archive(archive_path) as validated:
        if validated.layout != layout:
            msg = (
                "the archive changed between the verification scan and the "
                "extraction; refusing to write bytes the scan did not accept"
            )
            raise ExtractionError(msg)
        entry = _binary_entry(validated.archive)
        return _write_entry(validated.archive, entry, destination, layout)


def _require_trusted_bytes(archive_path: pathlib.Path, expected_sha256: str) -> None:
    """Require the archive's bytes to still match the trusted digest.

    Raises
    ------
    ExtractionError
        If the archive no longer hashes to ``expected_sha256``. The bytes are
        re-read rather than trusted from the caller's scan because the file
        may have been replaced between that scan and this write, and a
        replacement can match the scanned layout exactly.
    """
    observed = release_digest_files.digest_of_file(archive_path)
    if release_digest.constant_time_equal(observed, expected_sha256):
        return
    msg = (
        f"the archive's bytes no longer match the trusted digest: {observed}, "
        f"expected {expected_sha256}"
    )
    raise ExtractionError(msg)


def _require_fresh_directory(destination: pathlib.Path) -> None:
    """Require ``destination`` to be a directory that holds no entry.

    Raises
    ------
    ExtractionError
        If the path is not a directory, or already holds anything.
    """
    if not destination.is_dir():
        msg = f"{destination} is not a directory"
        raise ExtractionError(msg)
    if any(destination.iterdir()):
        msg = (
            f"{destination} is not empty; extraction writes only into a fresh directory"
        )
        raise ExtractionError(msg)


def _binary_entry(archive: tarfile.TarFile) -> tarfile.TarInfo:
    """Return the root binary's header, refusing an ambiguous match.

    Returns
    -------
    tarfile.TarInfo
        The header of the one regular member named like the shipped binary.

    Raises
    ------
    ExtractionError
        If the archive holds no unique regular root binary.

    Notes
    -----
    A member name that could escape the extraction root fails as a
    :class:`release_consumer_archive_policy.UnsafeMemberNameError`, which the
    re-scan raises before this runs; the check here only counts matches.
    """
    matches = [
        entry
        for entry in archive.getmembers()
        if policy.normalised_member_name(entry.name) == policy.BINARY_NAME
        and entry.isreg()
    ]
    if len(matches) != 1:
        msg = (
            f"expected one regular {policy.BINARY_NAME!r} member, found {len(matches)}"
        )
        raise ExtractionError(msg)
    return matches[0]


def _write_entry(
    archive: tarfile.TarFile,
    entry: tarfile.TarInfo,
    destination: pathlib.Path,
    layout: policy.ArchiveLayout,
) -> pathlib.Path:
    """Copy one member's data into a newly created file.

    Returns
    -------
    pathlib.Path
        The path of the written binary, which is executable.

    Notes
    -----
    The file is created by :func:`_created_exclusively`, which refuses an
    existing entry as :class:`ExtractionError`, and the bytes are copied by
    :func:`_copy_member`, which refuses a short or over-long member as
    :class:`ExtractionError` and damaged data as
    :class:`release_binstall_archive.ArchiveCorruptError`. The partial file is
    deleted either way.
    """
    target = destination / layout.binary_name
    descriptor = _created_exclusively(target)
    try:
        os.fchmod(descriptor, BINARY_MODE)
        _copy_member(archive, entry, descriptor, layout)
    except BaseException:
        _discard(target)
        raise
    finally:
        _closed(descriptor)
    return target


def _created_exclusively(target: pathlib.Path) -> int:
    """Open ``target`` for writing, refusing one that already exists or is a link.

    Returns
    -------
    int
        The descriptor of the newly created file.

    Raises
    ------
    ExtractionError
        If the file exists, is a symlink, or cannot be created.

    Notes
    -----
    ``O_EXCL`` and ``O_NOFOLLOW`` together mean a pre-existing entry cannot
    absorb the write, whether it is a file, a directory, or a link planted to
    redirect the extraction.
    """
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
    try:
        return os.open(target, flags, BINARY_MODE)
    except OSError as error:
        msg = f"could not create {target} exclusively: {error}"
        raise ExtractionError(msg) from error


def _discard(target: pathlib.Path) -> None:
    """Delete a partial output, ignoring a failure to remove it."""
    with contextlib.suppress(OSError):
        target.unlink(missing_ok=True)


def _closed(descriptor: int) -> None:
    """Close ``descriptor``, ignoring a failure to do so."""
    with contextlib.suppress(OSError):
        os.close(descriptor)


def _copy_member(
    archive: tarfile.TarFile,
    entry: tarfile.TarInfo,
    descriptor: int,
    layout: policy.ArchiveLayout,
) -> None:
    """Stream the member into ``descriptor``, reproducing the declared size.

    Raises
    ------
    ExtractionError
        If the member has no readable data or carries more or fewer bytes than
        its header declares.

    Notes
    -----
    Damaged compressed data surfaces as :class:`ArchiveCorruptError` from the
    steps that read it, before or during the copy.
    """
    source = _member_source(archive, entry, layout)
    written = _stream_member(source, descriptor, layout)
    if written != layout.binary_size:
        msg = (
            f"wrote {written} bytes for {layout.binary_name!r}, but the header "
            f"declares {layout.binary_size}"
        )
        raise ExtractionError(msg)


def _member_source(
    archive: tarfile.TarFile,
    entry: tarfile.TarInfo,
    layout: policy.ArchiveLayout,
) -> typ.BinaryIO:
    """Return the member's data stream, or refuse one that carries none.

    Returns
    -------
    typing.BinaryIO
        The stream the member's bytes are read from.

    Raises
    ------
    ExtractionError
        If the member has no readable data.
    ArchiveCorruptError
        If the archive's data could not be read at all.
    """
    try:
        source = archive.extractfile(entry)
    except CORRUPT_STREAM as error:
        msg = f"the archive's data could not be read: {error}"
        raise ArchiveCorruptError(msg) from error
    if source is None:
        msg = f"the archive member {layout.binary_name!r} carries no data"
        raise ExtractionError(msg)
    return source


def _stream_member(
    source: typ.BinaryIO, descriptor: int, layout: policy.ArchiveLayout
) -> int:
    """Copy the member's bytes into ``descriptor`` under the declared size.

    Returns
    -------
    int
        The number of bytes written, which the caller compares with the
        declared size.

    Raises
    ------
    ArchiveCorruptError
        If the compressed data fails part-way through.

    Notes
    -----
    A member that carries more bytes than its header declares is refused as
    :class:`ExtractionError` by :func:`_copy_under_cap`.
    """
    with source, os.fdopen(descriptor, "wb") as sink:
        try:
            return _copy_under_cap(source, sink, layout)
        except CORRUPT_STREAM as error:
            msg = f"the archive's data failed part-way through: {error}"
            raise ArchiveCorruptError(msg) from error


def _copy_under_cap(
    source: typ.BinaryIO, sink: typ.BinaryIO, layout: policy.ArchiveLayout
) -> int:
    """Copy ``source`` into ``sink``, refusing to write past the declared size.

    Returns
    -------
    int
        The number of bytes written.

    Raises
    ------
    ExtractionError
        If the member carries more bytes than its header declares.
    """
    written = 0
    while block := source.read(COPY_CHUNK_BYTES):
        written += len(block)
        if written > layout.binary_size:
            msg = (
                f"the archive member {layout.binary_name!r} carries more "
                "bytes than its header declares"
            )
            raise ExtractionError(msg)
        _ = sink.write(block)
    return written
