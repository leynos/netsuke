"""Exercise the staged-tree hashing the declaration stage performs.

The producer's job is narrow: given the names a release declares, hash the
staged file that publishes under each one. The tests here hold it to that,
including the two cases where the tree cannot answer — a declared asset that
was never staged, and one published name claimed by two files.
"""

import typing as typ

import pytest
import release_digest_files
from release_staged_digests import (
    AmbiguousStagedAssetError,
    UnstagedAssetError,
    staged_digests,
)

if typ.TYPE_CHECKING:
    import pathlib

ARCHIVE = "netsuke-build-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
PACKAGE = "netsuke_0.1.0-1_amd64.deb"
PRERELEASE_PACKAGE = "netsuke-0.1.0~rc1-1.x86_64.rpm"


def test_a_declared_asset_is_hashed_from_its_staged_file(
    tmp_path: pathlib.Path,
) -> None:
    """Return the digest of the bytes the build staged, keyed by name."""
    _ = (tmp_path / ARCHIVE).write_bytes(b"netsuke archive bytes")

    digests = staged_digests(tmp_path, expected=[ARCHIVE])

    assert digests == {
        ARCHIVE: release_digest_files.digest_of_file(tmp_path / ARCHIVE)
    }, "the recorded digest must describe the staged file's bytes"


def test_only_declared_assets_are_recorded(tmp_path: pathlib.Path) -> None:
    """Ignore the files a package ships internally, which are not assets."""
    _ = (tmp_path / ARCHIVE).write_bytes(b"archive")
    _ = (tmp_path / "netsuke").write_bytes(b"the binary inside the archive")
    _ = (tmp_path / "LICENCE").write_bytes(b"licence text")

    digests = staged_digests(tmp_path, expected=[ARCHIVE])

    assert sorted(digests) == [ARCHIVE], (
        f"only declared names belong in the record, got {sorted(digests)}"
    )


def test_a_nested_staged_package_is_found(tmp_path: pathlib.Path) -> None:
    """Walk below the root, because packages sit there until the hoist runs."""
    nested = tmp_path / "target" / "release"
    nested.mkdir(parents=True)
    _ = (nested / PACKAGE).write_bytes(b"package bytes")

    digests = staged_digests(tmp_path, expected=[PACKAGE])

    assert sorted(digests) == [PACKAGE], (
        f"a package below the staging root must still be found, got {sorted(digests)}"
    )


def test_a_prerelease_package_is_recorded_under_its_published_name(
    tmp_path: pathlib.Path,
) -> None:
    """Key the record by the name GitHub publishes, not the staged spelling.

    The tree holds a ``~``; GitHub publishes a ``.``. Recording the staged
    spelling would make every comparison against the download fail, and the
    failure would look like a substituted asset.
    """
    _ = (tmp_path / PRERELEASE_PACKAGE).write_bytes(b"prerelease package bytes")

    digests = staged_digests(tmp_path, expected=["netsuke-0.1.0.rc1-1.x86_64.rpm"])

    assert sorted(digests) == ["netsuke-0.1.0.rc1-1.x86_64.rpm"], (
        f"the record must use the published name, got {sorted(digests)}"
    )


def test_a_declared_asset_that_was_never_staged_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a configured target whose archive the build did not produce."""
    _ = (tmp_path / ARCHIVE).write_bytes(b"archive")

    with pytest.raises(UnstagedAssetError):
        _ = staged_digests(tmp_path, expected=[ARCHIVE, PACKAGE])


def test_one_published_name_claimed_by_two_files_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an ambiguity rather than letting one file silently win.

    Two files at different depths publish under one name. One of them would
    never be compared against anything, so which the release ships has to be
    decided before the record is written.
    """
    _ = (tmp_path / PRERELEASE_PACKAGE).write_bytes(b"staged spelling")
    (tmp_path / "nested").mkdir()
    _ = (tmp_path / "nested" / "netsuke-0.1.0.rc1-1.x86_64.rpm").write_bytes(
        b"published spelling"
    )

    with pytest.raises(AmbiguousStagedAssetError):
        _ = staged_digests(tmp_path, expected=["netsuke-0.1.0.rc1-1.x86_64.rpm"])


def test_a_repeated_declared_name_is_recorded_once(tmp_path: pathlib.Path) -> None:
    """Collapse a name declared twice rather than hashing it twice."""
    _ = (tmp_path / ARCHIVE).write_bytes(b"archive")

    digests = staged_digests(tmp_path, expected=[ARCHIVE, ARCHIVE])

    assert sorted(digests) == [ARCHIVE], (
        f"a repeated name must yield one entry, got {sorted(digests)}"
    )
