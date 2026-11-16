"""Test writing the downloaded archive's binary out to a fresh directory.

Extraction is judged before the first byte is written, and the bytes are bound
to the digest the archive was scanned with, so the cases here cover both. The
fixtures come from :mod:`release_binstall_fixtures`.

Run via ``make test-release-publication``.
"""

import pathlib
import typing as typ

import pytest
import release_binstall_archive as adapter
import release_binstall_extract as extract
import release_consumer_archive_policy as policy
from release_binstall_fixtures import BINARY_BYTES, digest_of, tarball


def test_extract_binary_writes_only_the_root_binary(
    tmp_path: pathlib.Path,
) -> None:
    """Write the binary executable and leave every other member unwritten."""
    archive = tarball(tmp_path / "archive.tar.gz", extra=[("LICENSE", b"MIT")])
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))
    destination = tmp_path / "out"
    destination.mkdir()

    binary = extract.extract_binary(
        archive, layout, destination, expected_sha256=digest_of(archive)
    )

    assert binary.read_bytes() == BINARY_BYTES, (
        "the extracted bytes must be the member's bytes"
    )
    assert binary.stat().st_mode & 0o777 == 0o755, (
        "the extracted binary must be executable"
    )
    assert [path.name for path in destination.iterdir()] == [policy.BINARY_NAME], (
        "only the root binary may be written"
    )


def test_extract_binary_refuses_a_destination_holding_a_symlink(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse to write into a directory that already holds an entry."""
    archive = tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))
    destination = tmp_path / "out"
    destination.mkdir()
    elsewhere = tmp_path / "elsewhere"
    _ = elsewhere.write_bytes(b"untouched")
    (destination / policy.BINARY_NAME).symlink_to(elsewhere)

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, layout, destination, expected_sha256=digest_of(archive)
        )

    assert elsewhere.read_bytes() == b"untouched", (
        "a planted symlink must not redirect the write"
    )


def test_extract_binary_refuses_a_missing_destination(tmp_path: pathlib.Path) -> None:
    """Refuse a destination the caller never created."""
    archive = tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, layout, tmp_path / "absent", expected_sha256=digest_of(archive)
        )


def test_extract_binary_refuses_a_layout_from_other_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an archive that changed after it was scanned."""
    archive = tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))
    stale = policy.ArchiveLayout(
        binary_name=layout.binary_name,
        binary_size=layout.binary_size + 1,
        member_count=layout.member_count,
    )
    destination = tmp_path / "out"
    destination.mkdir()

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, stale, destination, expected_sha256=digest_of(archive)
        )

    assert not list(destination.iterdir()), (
        "a refused extraction must leave the directory empty"
    )


def test_extract_binary_refuses_a_replacement_that_keeps_the_layout(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a replacement archive whose members still match the scan.

    ``ArchiveLayout`` records the binary's name, its declared size and the
    member count, not the content, so a replacement of equal size passes the
    scan. The trusted digest is what separates the two archives, and this
    test changes only the bytes: the file is replaced, the layout is not. The
    refusal is the digest's own, because extraction and verification share one
    open archive and the mismatch is found while that archive is hashed.
    """
    archive = tarball(tmp_path / "archive.tar.gz")
    trusted = digest_of(archive)
    layout = adapter.scan_archive(archive, expected_sha256=trusted)
    replacement = tarball(
        tmp_path / "replacement.tar.gz", binary=b"netsuke binary payloaD"
    )
    _ = replacement.replace(archive)

    assert (
        adapter.scan_archive(archive, expected_sha256=digest_of(archive)) == layout
    ), "the fixture must be one the layout re-scan accepts, or it proves nothing"
    destination = tmp_path / "out"
    destination.mkdir()

    with pytest.raises(adapter.ArchiveDigestMismatchError):
        extract.extract_binary(archive, layout, destination, expected_sha256=trusted)

    assert not list(destination.iterdir()), (
        "a refused extraction must leave the directory empty"
    )


def test_extract_binary_opens_the_archive_once(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Hash and parse one open archive rather than a pathname opened twice.

    Two opens leave a window between the digest check and the scan in which
    the name can resolve to a different archive whose members still match the
    accepted layout. Opening once removes the window, so this counts opens.
    """
    archive = tarball(tmp_path / "archive.tar.gz")
    digest = digest_of(archive)
    layout = adapter.scan_archive(archive, expected_sha256=digest)
    destination = tmp_path / "out"
    destination.mkdir()
    opened: list[pathlib.Path] = []
    original = pathlib.Path.open

    def counting(target: pathlib.Path, mode: str) -> typ.IO[bytes]:
        """Record one open and read the file the adapter asked for."""
        opened.append(target)
        return original(target, mode)

    monkeypatch.setattr(pathlib.Path, "open", counting)

    _ = extract.extract_binary(archive, layout, destination, expected_sha256=digest)

    assert opened == [archive], (
        "extraction must hash and parse the archive through a single open"
    )


def test_extract_binary_removes_a_partial_file_when_the_copy_fails(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Delete the partial output when the copy cannot finish.

    A gzip stream that fails part-way through cannot be built without also
    breaking the member index the scan reads first, so the copy step is failed
    directly: the property under test is the cleanup, not how tar reports a
    damaged stream.
    """

    def explode(*_args: object, **_kwargs: object) -> None:
        """Fail the copy the way a damaged stream would."""
        msg = "stream failed part-way through"
        raise adapter.ArchiveCorruptError(msg)

    archive = tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))
    destination = tmp_path / "out"
    destination.mkdir()
    monkeypatch.setattr(extract, "_copy_member", explode)

    with pytest.raises(adapter.ArchiveCorruptError):
        extract.extract_binary(
            archive, layout, destination, expected_sha256=digest_of(archive)
        )

    assert not list(destination.iterdir()), (
        "a failed extraction must not leave a partial binary behind"
    )


def test_extract_binary_translates_a_write_failure(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Report a failed write as an archive refusal rather than a bare OSError.

    A full or read-only filesystem surfaces here as :class:`OSError`, which
    the smoke's exit-code table does not classify. Left untranslated it would
    end the run with a traceback instead of the archive-refused status.
    """

    def no_space(*_args: object, **_kwargs: object) -> None:
        """Fail the copy the way a full filesystem does."""
        msg = "No space left on device"
        raise OSError(msg)

    archive = tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))
    destination = tmp_path / "out"
    destination.mkdir()
    monkeypatch.setattr(extract, "_copy_member", no_space)

    with pytest.raises(adapter.ExtractionError):
        extract.extract_binary(
            archive, layout, destination, expected_sha256=digest_of(archive)
        )

    assert not list(destination.iterdir()), (
        "a refused write must leave the directory as it found it"
    )


def test_extract_binary_reports_an_absolute_path_for_a_relative_destination(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Write and report one entry, however the destination was named.

    The destination is resolved once, before it is checked, so a caller that
    passed a relative path still receives the absolute path of the file that
    was written. Resolving the write and the report separately would let the
    two name different entries.
    """
    monkeypatch.chdir(tmp_path)
    archive = tarball(tmp_path / "archive.tar.gz")
    layout = adapter.scan_archive(archive, expected_sha256=digest_of(archive))
    destination = pathlib.Path("out")
    destination.mkdir()

    written = extract.extract_binary(
        archive, layout, destination, expected_sha256=digest_of(archive)
    )

    assert written.is_absolute(), (
        "the returned path must not depend on the caller's working directory"
    )
    assert written == tmp_path / "out" / policy.BINARY_NAME, (
        "the returned path must name the file the extraction wrote"
    )
    assert written.read_bytes() == BINARY_BYTES, (
        "the file at the returned path must be the archive's binary"
    )
