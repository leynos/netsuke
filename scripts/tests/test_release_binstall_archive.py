"""Test reading and verifying a downloaded archive.

Archives are built in memory and written to a temporary directory, so no test
here reaches the network. The fixtures come from
:mod:`release_binstall_fixtures`, which also serves the extraction and probe
suites.

Run via ``make test-release-publication``.
"""

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


def test_scan_archive_refuses_a_link_member(tmp_path: pathlib.Path) -> None:
    """Refuse an archive whose binary member is a symlink."""
    archive = tarball(tmp_path / "archive.tar.gz", kind=policy.MemberKind.SYMLINK)

    with pytest.raises(policy.UnsafeMemberTypeError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))
