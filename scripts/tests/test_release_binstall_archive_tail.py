"""Hold the archive adapter to reading the index through to its end.

``tarfile`` reports nothing about the blocks it stops at: a second archive
appended after the end marker is never parsed, and a header whose checksum is
wrong ends the walk silently once the first member has been accepted. The
adapter reads the decompressed stream again from the last accepted member, so
these tests build archives whose damage sits exactly there and require the
adapter to refuse them, while a standard archive padded with zero blocks still
passes.

Run via ``make test-release-publication``.
"""

import gzip
import io
import tarfile
import typing as typ

import pytest
import release_binstall_archive as adapter
import release_consumer_archive_policy as policy
from release_binstall_fixtures import BINARY_BYTES, digest_of

if typ.TYPE_CHECKING:
    import pathlib


def _tar_bytes() -> bytes:
    """Return an uncompressed archive holding one root binary.

    Returns
    -------
    bytes
        The tar stream, including the end marker's zero blocks.
    """
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w") as archive:
        entry = tarfile.TarInfo(policy.BINARY_NAME)
        entry.size = len(BINARY_BYTES)
        _ = archive.addfile(entry, io.BytesIO(BINARY_BYTES))
    return buffer.getvalue()


def _marker_offset(stream: bytes) -> int:
    """Return the offset of the first all-zero block in ``stream``.

    Returns
    -------
    int
        Where the archive's end marker begins, which is also where the member
        walk stops accepting members.
    """
    return next(
        offset
        for offset in range(0, len(stream), tarfile.BLOCKSIZE)
        if stream[offset : offset + tarfile.BLOCKSIZE] == b"\0" * tarfile.BLOCKSIZE
    )


def _gzipped(path: pathlib.Path, blocks: bytes) -> pathlib.Path:
    """Write ``blocks`` as one gzip-compressed download.

    Returns
    -------
    pathlib.Path
        The archive, which is the path it was asked to write.
    """
    _ = path.write_bytes(gzip.compress(blocks))
    return path


def _gzipped_archive(path: pathlib.Path, tail: bytes) -> pathlib.Path:
    """Write an archive whose decompressed stream continues past its end.

    Returns
    -------
    pathlib.Path
        The archive, which is the path it was asked to write.
    """
    return _gzipped(path, _tar_bytes() + tail)


def _gzipped_index(path: pathlib.Path, block: bytes) -> pathlib.Path:
    """Write an archive whose index runs into ``block`` before its end marker.

    Returns
    -------
    pathlib.Path
        The archive, which is the path it was asked to write.
    """
    stream = _tar_bytes()
    marker = _marker_offset(stream)
    return _gzipped(path, stream[:marker] + block + b"\0" * 2 * tarfile.BLOCKSIZE)


def _mis_checksummed_header() -> bytes:
    """Return a header block whose fields no longer match its checksum.

    Returns
    -------
    bytes
        A copy of the archive's own header with its first name byte changed.
        The recorded checksum still describes the original name, so the block
        is an invalid header rather than one the parser can read.
    """
    header = bytearray(_tar_bytes()[: tarfile.BLOCKSIZE])
    header[0:1] = b"Z"
    return bytes(header)


def test_scan_archive_accepts_zero_padding_after_the_end_marker(
    tmp_path: pathlib.Path,
) -> None:
    """Accept the zero padding standard writers leave after the end marker."""
    archive = _gzipped_archive(tmp_path / "archive.tar.gz", b"\0" * tarfile.BLOCKSIZE)

    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))

    assert layout.member_count == 1, "the padding must not be read as a member"


def test_scan_archive_refuses_data_after_the_end_marker(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a block after the end marker, which the index walk never parses.

    ``tarfile`` stops at the marker, so a block that follows it — a second
    archive, or one whose checksum is wrong — is never read by the walk, and a
    scan that trusted the walk would accept it. Only reading the decompressed
    stream past the marker sees it.
    """
    archive = _gzipped_archive(tmp_path / "trail.tar.gz", b"\xaa" * tarfile.BLOCKSIZE)

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))


def test_scan_archive_refuses_a_header_the_walk_ends_on_silently(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a bad-checksum header standing where the end marker belongs.

    Past the first block, ``tarfile`` ends the walk on an invalid header
    without raising, and it has consumed that block by the time it returns. The
    index therefore looks like one valid member followed by padding, and only a
    scan that reads the block again from the last accepted offset sees it. The
    trailing zero blocks prove the refusal is the bad header rather than an
    archive that merely lacks a marker.
    """
    archive = _gzipped_index(tmp_path / "bad-header.tar.gz", _mis_checksummed_header())

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))
