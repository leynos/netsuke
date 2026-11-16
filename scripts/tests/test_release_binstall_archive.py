"""Test reading and verifying a downloaded archive.

Archives are built in memory and written to a temporary directory, so no test
here reaches the network. The fixtures come from
:mod:`release_binstall_fixtures`, which also serves the extraction and probe
suites.

Run via ``make test-release-publication``.
"""

import hashlib
import io
import tarfile
import typing as typ

import pytest
import release_binstall_archive as adapter
import release_consumer_archive_policy as policy
from release_binstall_fixtures import BINARY_BYTES, digest_of, tarball

if typ.TYPE_CHECKING:
    import pathlib


def test_scan_archive_accepts_a_well_formed_archive(tmp_path: pathlib.Path) -> None:
    """Return the validated layout of an archive whose digest matches."""
    archive = tarball(tmp_path / "archive.tar.gz", extra=[("LICENSE", b"MIT")])

    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))

    assert (layout.binary_name, layout.binary_size) == (
        policy.BINARY_NAME,
        len(BINARY_BYTES),
    ), "the scan must report the root binary and its declared size"
    assert layout.member_count == 2, "the scan must count every member"


def test_scan_archive_refuses_an_empty_file(tmp_path: pathlib.Path) -> None:
    """Refuse a download that carries no bytes."""
    archive = tmp_path / "empty.tar.gz"
    _ = archive.write_bytes(b"")

    with pytest.raises(adapter.ArchiveEmptyError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))


def test_scan_archive_refuses_an_oversized_file(tmp_path: pathlib.Path) -> None:
    """Refuse a file larger than the fixed read cap, without reading it."""
    archive = tmp_path / "huge.tar.gz"
    with archive.open("wb") as handle:
        handle.seek(adapter.MAX_ARCHIVE_BYTES)
        _ = handle.write(b"x")

    with pytest.raises(adapter.ArchiveFileTooLargeError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))


def test_scan_archive_refuses_a_differentdigest_of(tmp_path: pathlib.Path) -> None:
    """Refuse bytes the trusted record does not name."""
    archive = tarball(tmp_path / "archive.tar.gz")

    with pytest.raises(adapter.ArchiveDigestMismatchError):
        adapter.scan_archive(archive, expected_sha256="ab" * 32)


def test_scan_archive_refuses_bytes_that_are_not_atarball(
    tmp_path: pathlib.Path,
) -> None:
    """Translate a gzip or tar failure into one corrupt-archive refusal."""
    archive = tmp_path / "archive.tar.gz"
    _ = archive.write_bytes(b"not a gzip stream at all")

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))


def test_scan_archive_refuses_a_truncated_archive(tmp_path: pathlib.Path) -> None:
    """Refuse a stream that ends before the archive does."""
    archive = tarball(tmp_path / "archive.tar.gz")
    truncated = tmp_path / "truncated.tar.gz"
    _ = truncated.write_bytes(archive.read_bytes()[: len(archive.read_bytes()) // 2])

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(truncated, expected_sha256=digest_of(truncated))


def _incompressible_payload(size: int) -> bytes:
    """Return ``size`` bytes that deflate cannot shrink.

    Returns
    -------
    bytes
        Exactly ``size`` bytes, taken from a hash chain rather than a
        generator, so the fixture is identical on every run.
    """
    blocks: list[bytes] = []
    seed = b"netsuke"
    while len(blocks) * 32 < size:
        seed = hashlib.sha256(seed).digest()
        blocks.append(seed)
    return b"".join(blocks)[:size]


def test_scan_archive_refuses_an_over_long_index_before_reading_it(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse past the member cap without walking the rest of the index.

    The fixture carries more members than the cap allows and its tail is cut
    off, so a scan that read the whole index would report a corrupt archive.
    Only a scan that stops once the cap is passed can report the member-count
    refusal, and the last assertion proves the fixture is damaged enough for
    that distinction to mean something.
    """
    archive = _truncated_index(tmp_path / "archive.tar.gz")
    digest = digest_of(archive)

    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(policy, "MAX_MEMBERS", 1)
        with pytest.raises(policy.ArchiveTooLargeError):
            adapter.scan_archive(archive, expected_sha256=digest)

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(archive, expected_sha256=digest)


def _truncated_index(path: pathlib.Path) -> pathlib.Path:
    """Write an archive of incompressible members and cut its index short.

    Returns
    -------
    pathlib.Path
        The truncated archive, which is the path it was asked to write.
    """
    payload = _incompressible_payload(64 * 1024)
    with tarfile.open(path, "w:gz") as archive:
        for index in range(4):
            entry = tarfile.TarInfo(f"member-{index}")
            entry.size = len(payload)
            _ = archive.addfile(entry, io.BytesIO(payload))
    raw = path.read_bytes()
    _ = path.write_bytes(raw[: len(raw) // 2])
    return path


def test_scan_archive_refuses_a_link_member(tmp_path: pathlib.Path) -> None:
    """Refuse an archive whose binary member is a symlink."""
    archive = tarball(tmp_path / "archive.tar.gz", kind=policy.MemberKind.SYMLINK)

    with pytest.raises(policy.UnsafeMemberTypeError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))
