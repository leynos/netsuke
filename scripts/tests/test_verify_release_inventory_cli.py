"""Exercise inventory verification over staged release assets.

These tests call ``verify()`` directly to check inventory, sidecar, and trusted
digest agreement. The CLI's file-input and exit-code contract lives in
``test_verify_release_inventory_command.py``.
"""

import typing as typ

import pytest
import release_digest
import release_digest_files
import release_digest_record
import release_inventory
from release_staging_support import ARCHIVE, PACKAGE, SIDECAR
from release_staging_support import stage_archive as _stage
from release_staging_support import stage_package as _stage_package
from release_staging_support import trusted_digests as _trusted
from verify_release_inventory_cli import verify

if typ.TYPE_CHECKING:
    import pathlib


def _record(digests: dict[str, str]) -> release_digest_record.DigestRecord:
    """Build the trusted record an adapter decodes from a written payload."""
    return release_digest_record.DigestRecord.from_mapping(digests)


def test_verify_accepts_the_declared_assets(tmp_path: pathlib.Path) -> None:
    """Accept an archive with a matching sidecar and report each asset's own digest.

    The sidecar entry is asserted separately and deliberately. Returning the
    archive's digest under the sidecar's name type-checks and looks right in a
    two-key mapping, but GitHub reports a digest for the sidecar file itself,
    so the finalizer would compare the wrong value. Computing the two digests
    from the staged files rather than from the CLI's own output keeps the
    assertion independent of the code under test.
    """
    staging = tmp_path / "dist"
    _stage(staging)

    verified = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=_trusted(staging))

    assert sorted(verified) == [ARCHIVE, SIDECAR], (
        f"both assets must be keyed by name, got {sorted(verified)}"
    )
    assert verified[ARCHIVE] == release_digest_files.digest_of_file(
        staging / ARCHIVE
    ), "the archive's digest must be the digest of its bytes"
    assert verified[SIDECAR] == release_digest_files.digest_of_file(
        staging / SIDECAR
    ), (
        "the sidecar's digest must be the digest of the sidecar file, not of "
        "the archive it describes"
    )
    assert verified[SIDECAR] != verified[ARCHIVE], (
        "the two entries must differ, or this assertion cannot tell the bug "
        "from the fix"
    )


def test_verify_reports_a_missing_archive(tmp_path: pathlib.Path) -> None:
    """Reject a release carrying a sidecar whose archive never arrived.

    This is the 2026-10-12 failure: every staged file passed inspection, and
    the release was unusable because the archives were the files that were
    missing. The declaration is what makes the absence visible.
    """
    staging = tmp_path / "dist"
    staging.mkdir()
    _ = (staging / SIDECAR).write_text("ab  dist/x\n", encoding="utf-8")

    with pytest.raises(release_inventory.MissingAssetError):
        _ = verify(
            staging,
            expected=[ARCHIVE, SIDECAR],
            trusted=_record({ARCHIVE: "ab" * 32, SIDECAR: "ab" * 32}),
        )


def test_verify_reports_an_unexpected_archive(tmp_path: pathlib.Path) -> None:
    """Reject an asset the release never declared."""
    staging = tmp_path / "dist"
    _stage(staging)
    _ = (staging / "extra.tar.gz").write_bytes(b"extra")

    with pytest.raises(release_inventory.UnexpectedAssetError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=_trusted(staging))


def test_verify_rejects_a_sidecar_describing_different_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when the sidecar's digest is not the archive's, not merely well formed."""
    staging = tmp_path / "dist"
    _stage(staging, sidecar_body=b"cd" * 32)

    with pytest.raises(release_digest.DigestMismatchError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=_trusted(staging))


def test_verify_rejects_a_replaced_operating_system_package(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when a downloaded package differs from the bytes the build staged.

    The package is replaced outright. Nothing inside the download disagrees —
    a package has no sidecar — so this is the case only the trusted record can
    see, and it is the reason that record exists.
    """
    staged = tmp_path / "staged"
    _stage_package(staged, b"the package the build produced")
    trusted = _trusted(staged)

    downloaded = tmp_path / "downloaded"
    _stage_package(downloaded, b"a package somebody else produced")

    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        _ = verify(downloaded, expected=[PACKAGE], trusted=trusted)


def test_verify_rejects_a_replaced_archive_and_sidecar_pair(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when an archive and its sidecar are replaced together.

    Both files are rewritten so the sidecar still describes the archive beside
    it. The two older agreements — inventory exactness and sidecar agreement —
    both hold over this download, which is what makes the pair-swap the case
    those agreements cannot detect and the trusted record must.
    """
    staged = tmp_path / "staged"
    _stage(staged)
    trusted = _trusted(staged)

    downloaded = tmp_path / "downloaded"
    _stage(downloaded)
    replaced = b"a different archive entirely"
    _ = (downloaded / ARCHIVE).write_bytes(replaced)
    digest = release_digest_files.digest_of_file(downloaded / ARCHIVE)
    _ = (downloaded / SIDECAR).write_text(
        f"{digest}  dist/{ARCHIVE}\n", encoding="utf-8"
    )

    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        _ = verify(downloaded, expected=[ARCHIVE, SIDECAR], trusted=trusted)


def test_verify_rejects_a_record_that_omits_a_declared_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a trusted record that leaves an asset uncompared.

    A record covering only part of the declaration would let the omitted asset
    through whatever its bytes are, so the omission is the failure.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    partial = _record({ARCHIVE: _trusted(staging).as_mapping()[ARCHIVE]})

    with pytest.raises(release_digest_record.DigestRecordError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=partial)
