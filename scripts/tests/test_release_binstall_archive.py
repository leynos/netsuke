"""Test reading and verifying a downloaded archive.

Archives are built in memory and written to a temporary directory, so no test
here reaches the network. The fixtures come from
:mod:`release_binstall_fixtures`, which also serves the extraction and probe
suites.

Run via ``make test-release-publication``.
"""

import gzip
import hashlib
import io
import tarfile
import typing as typ

import pytest
import release_binstall_archive as adapter
import release_binstall_inflate as inflate
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


def test_scan_archive_refuses_a_missing_gzip_trailer(tmp_path: pathlib.Path) -> None:
    """Refuse a download truncated after the archive's own end.

    The tar member index is complete, so nothing the archive itself parses
    reaches the missing bytes; only reading the compressed stream to its end
    makes gzip notice that its trailer never arrived.
    """
    archive = tarball(tmp_path / "archive.tar.gz")
    truncated = tmp_path / "truncated.tar.gz"
    _ = truncated.write_bytes(archive.read_bytes()[:-8])

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(truncated, expected_sha256=digest_of(truncated))


def test_scan_archive_refuses_an_incorrect_gzip_crc(tmp_path: pathlib.Path) -> None:
    """Refuse a download whose gzip trailer records the wrong checksum."""
    archive = tarball(tmp_path / "archive.tar.gz")
    trailer = bytearray(archive.read_bytes())
    trailer[-8] ^= 0xFF
    damaged = tmp_path / "damaged.tar.gz"
    _ = damaged.write_bytes(trailer)

    with pytest.raises(adapter.ArchiveCorruptError):
        adapter.scan_archive(damaged, expected_sha256=digest_of(damaged))


def test_inflate_refuses_an_oversized_read_without_allocating_it() -> None:
    """Serve an oversized read in chunks and refuse it at the budget.

    A pax header carries the size of its own body, and the inherited
    ``RawIOBase.read`` allocates the whole request before ``readinto`` can
    refuse it. A request larger than any allocation the host would grant must
    still surface as the budget's refusal, which shows the request was served
    in chunks rather than allocated whole.
    """
    stream = inflate.InflatedStream(
        io.BytesIO(gzip.compress(b"netsuke" * 4)), limit=16, refusal=ValueError
    )

    with stream, pytest.raises(ValueError, match="more than the 16 bytes"):
        _ = stream.read(1 << 48)


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


def test_scan_archive_refuses_a_header_that_declares_more_than_the_cap(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a stream whose extended header claims more than the budget.

    The long member name travels in an extended PAX header, whose declared size
    ``tarfile`` reads before it can produce a member record, so the expansion is
    measured while the header is read — and the size declared there is beyond
    the budget. The smaller archive must still scan under the same budget, so
    the refusal is the budget rather than the patched limit.
    """
    short = tarball(tmp_path / "short.tar.gz")
    long = _long_named_archive(tmp_path / "long.tar.gz")
    cap = len(gzip.decompress(short.read_bytes())) + tarfile.BLOCKSIZE
    declared = _declared_header_size(long)
    assert declared is not None, "the fixture must carry an extended header"
    assert declared > cap, (
        "the fixture's declared header size must exceed the budget under test"
    )

    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(inflate, "MAX_INFLATED_BYTES", cap)
        assert (
            adapter.scan_archive(short, expected_sha256=digest_of(short)).member_count
            == 1
        ), "the budget must admit an archive of the shipped shape"
        with pytest.raises(adapter.ArchiveInflationError):
            adapter.scan_archive(long, expected_sha256=digest_of(long))


def _declared_header_size(path: pathlib.Path) -> int | None:
    """Return the body size the archive's first extended header declares.

    Returns
    -------
    int | None
        The byte count recorded in the header block for an extended or global
        pax header, or ``None`` when the archive carries none.
    """
    blocks = gzip.decompress(path.read_bytes())
    for offset in range(0, len(blocks), tarfile.BLOCKSIZE):
        block = blocks[offset : offset + tarfile.BLOCKSIZE]
        if block[156:157] in {b"x", b"g"}:
            return int(block[124:136].rstrip(b"\0 ") or b"0", 8)
    return None


def _long_named_archive(path: pathlib.Path) -> pathlib.Path:
    """Write a valid archive carrying one member with a very long name.

    Returns
    -------
    pathlib.Path
        The archive, which is the path it was asked to write.
    """
    name = f"man/{'netsuke' * 5000}.1"
    payload = b"page"
    with tarfile.open(path, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        entry = tarfile.TarInfo(policy.BINARY_NAME)
        entry.size = len(BINARY_BYTES)
        _ = archive.addfile(entry, io.BytesIO(BINARY_BYTES))
        page = tarfile.TarInfo(name)
        page.size = len(payload)
        _ = archive.addfile(page, io.BytesIO(payload))
    return path


def test_scan_archive_refuses_a_link_member(tmp_path: pathlib.Path) -> None:
    """Refuse an archive whose binary member is a symlink."""
    archive = tarball(tmp_path / "archive.tar.gz", kind=policy.MemberKind.SYMLINK)

    with pytest.raises(policy.UnsafeMemberTypeError):
        adapter.scan_archive(archive, expected_sha256=digest_of(archive))
