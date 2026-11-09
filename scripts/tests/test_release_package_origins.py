"""Test the observation that attributes each package to a producer's subtree.

A package name says its format and nothing about the target that built it, so
two same-platform targets can trade one package each and leave every count and
the release's union identical. What distinguishes them is *where* the bytes
were downloaded: every producer uploads one workflow artefact, the release
downloads each into its own top-level directory, and the producer writes its
record inside the tree it uploaded.

Each case here lays out a tree of that shape and asserts the attribution, so
the policy that consumes it can refuse an exchange rather than counting
formats.

The records themselves are placeholders. This module is handed a record path
and the target its reader found there, so it never opens one; the record's
contents are the reader's business and are covered where the reader lives.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_package_origins as origins
from record_producer_provenance_record import RECORD_NAME

if typ.TYPE_CHECKING:
    import pathlib

X86 = "linux-x86_64"
ARM = "linux-aarch64"
X86_DEB = "netsuke_1.0.0_amd64.deb"
ARM_DEB = "netsuke_1.0.0_arm64.deb"
X86_ARCHIVE = "netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz"


def place_record(directory: pathlib.Path) -> pathlib.Path:
    """Write a placeholder producer record into ``directory``.

    Returns
    -------
    pathlib.Path
        The path the caller would pass as a key of ``target_of_record``.
    """
    directory.mkdir(parents=True, exist_ok=True)
    record = directory / RECORD_NAME
    _ = record.write_text("{}", encoding="utf-8")
    return record


def place_asset(directory: pathlib.Path, name: str) -> None:
    """Write a placeholder file under ``name`` below ``directory``."""
    directory.mkdir(parents=True, exist_ok=True)
    _ = (directory / name).write_bytes(b"bytes")


def test_a_package_is_attributed_to_the_subtree_that_staged_it(
    tmp_path: pathlib.Path,
) -> None:
    """Attribute each package to the target whose record shares its subtree."""
    x86_record = place_record(tmp_path / "x86")
    arm_record = place_record(tmp_path / "arm")
    place_asset(tmp_path / "x86", X86_DEB)
    place_asset(tmp_path / "arm", ARM_DEB)
    observed = origins.observe_package_origins(
        tmp_path, target_of_record={x86_record: X86, arm_record: ARM}
    )
    assert observed == {X86_DEB: X86, ARM_DEB: ARM}, (
        "each package must be attributed to the target that staged it"
    )


def test_a_package_below_a_nested_directory_is_still_attributed(
    tmp_path: pathlib.Path,
) -> None:
    """Attribute a package found deeper than one directory below the subtree.

    A producer's artefact arrives as a downloaded tree, and where inside it the
    packaging step placed the package is not something this reads. Only the
    first segment names the producer, so a deeper path must attribute the same
    way.
    """
    record = place_record(tmp_path / "x86")
    place_asset(tmp_path / "x86" / "nested" / "deeper", X86_DEB)
    assert origins.observe_package_origins(
        tmp_path, target_of_record={record: X86}
    ) == {X86_DEB: X86}, "a nested package must be attributed to its subtree"


def test_an_archive_is_not_attributed(tmp_path: pathlib.Path) -> None:
    """Leave archives to the configuration, which binds them by name.

    A producer's subtree also holds its archive and sidecar. Those are hoisted
    to the ``dist`` root and matched against the configured archive name, so
    reporting an origin for them would be a second, weaker account of a fact
    the configuration already states exactly.
    """
    record = place_record(tmp_path / "x86")
    place_asset(tmp_path / "x86", X86_ARCHIVE)
    place_asset(tmp_path / "x86", X86_DEB)
    assert origins.observe_package_origins(
        tmp_path, target_of_record={record: X86}
    ) == {X86_DEB: X86}, "an archive must not be attributed by position"


def test_a_package_staged_twice_is_not_attributed(tmp_path: pathlib.Path) -> None:
    """Leave an ambiguous package unblamed rather than choosing a producer.

    Two subtrees holding one package name is the staged tree's own refusal.
    Picking either here would name a producer for an ambiguity no record can
    cause, which sends the operator to the wrong job.
    """
    first = place_record(tmp_path / "a")
    second = place_record(tmp_path / "b")
    place_asset(tmp_path / "a", X86_DEB)
    place_asset(tmp_path / "b", X86_DEB)
    assert (
        origins.observe_package_origins(
            tmp_path, target_of_record={first: X86, second: ARM}
        )
        == {}
    ), "an ambiguously staged package must not be attributed"


def test_a_record_at_the_dist_root_attributes_nothing(tmp_path: pathlib.Path) -> None:
    """Attribute nothing to a record that names no downloaded artefact.

    The hoist leaves archives at the ``dist`` root, so a tree can legitimately
    hold a record there. It names no producer subtree, so anything beside it
    is not evidence of a target.
    """
    record = place_record(tmp_path)
    place_asset(tmp_path, X86_DEB)
    assert (
        origins.observe_package_origins(tmp_path, target_of_record={record: X86}) == {}
    ), "a root-level record must attribute nothing"


def test_a_record_outside_the_dist_root_is_ignored(tmp_path: pathlib.Path) -> None:
    """Ignore a record that names a path outside the tree being observed.

    A caller threading the wrong root through must not have a sibling
    directory's packages attributed to this release.
    """
    dist = tmp_path / "dist"
    outsider = place_record(tmp_path / "elsewhere")
    place_asset(tmp_path / "elsewhere", X86_DEB)
    dist.mkdir()
    assert (
        origins.observe_package_origins(dist, target_of_record={outsider: X86}) == {}
    ), "a record outside the observed root must attribute nothing"


def test_an_unreadable_subtree_is_propagated(tmp_path: pathlib.Path) -> None:
    """Surface a traversal failure rather than reporting no packages.

    A swallowed error would look exactly like a subtree with no packages, and
    a producer whose artefact failed to download would then be blamed for
    claiming packages it did in fact stage. The record is named here but never
    written, so its subtree is the one that cannot be walked.
    """
    absent = tmp_path / "absent" / RECORD_NAME
    with pytest.raises(OSError, match="absent"):
        origins.observe_package_origins(tmp_path, target_of_record={absent: X86})
