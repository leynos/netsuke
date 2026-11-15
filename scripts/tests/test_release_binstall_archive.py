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
from release_binstall_fixtures import BINARY_BYTES, SIDECAR_NAME, digest_of, tarball

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


def test_verify_sidecar_accepts_a_matching_checksum(tmp_path: pathlib.Path) -> None:
    """Return the sidecar's digest when it describes the archive."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    digest = digest_of(archive)
    _ = sidecar.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")

    assert adapter.verify_sidecar(archive, sidecar) == digest, (
        "a matching sidecar must return the archive's digest"
    )


@pytest.mark.parametrize(
    "text",
    [
        "",
        "not a checksum\n",
        "ab" * 32 + "  another-asset.tar.gz\n",
        "cd" * 32 + "  archive.tar.gz\n",
    ],
    ids=["empty", "malformed", "other-name", "other-digest"],
)
def test_verify_sidecar_refuses_a_sidecar_that_does_not_describe_the_archive(
    tmp_path: pathlib.Path, text: str
) -> None:
    """Refuse a sidecar that is empty, malformed, or names other bytes."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_text(text, encoding="utf-8")

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_an_oversized_body(tmp_path: pathlib.Path) -> None:
    """Refuse a sidecar larger than one checksum line."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_bytes(b"a" * (adapter.MAX_SIDECAR_BYTES + 1))

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_a_non_utf8_body(tmp_path: pathlib.Path) -> None:
    """Refuse bytes that are not text at all."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_bytes(b"\xff\xfe\x00")

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_an_absent_sidecar(tmp_path: pathlib.Path) -> None:
    """Refuse a download that never arrived."""
    archive = tarball(tmp_path / "archive.tar.gz")

    with pytest.raises(adapter.SidecarVerificationError):
        adapter.verify_sidecar(archive, tmp_path / SIDECAR_NAME)
