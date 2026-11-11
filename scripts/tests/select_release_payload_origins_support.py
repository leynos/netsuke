"""Stage two Linux producers so cases can exchange their package claims.

A package's origin is the producer subtree the download placed it in, so the
only way to test that selection reads an origin is to stage two producers and
move a file from one subtree to the other. The refusal is the interesting one:
a moved package leaves the per-format count, the formats, and the union of
published names all unchanged, so only the origin attribute distinguishes it.

This module supplies that tree once. It lives apart from
``test_select_release_payload_cli`` because the staging, the configuration, and
the record building are a fixture rather than a case, and the case module is
already at its line budget.

Run via ``make test-release-publication``.
"""

import typing as typ

import release_digest_files
from select_release_payload_support import (
    DEB,
    MANIFEST,
    RPM,
    Provenance,
    StagedTree,
    record_provenance,
    stage,
)

if typ.TYPE_CHECKING:
    import pathlib

#: Two Linux targets, so a package can be moved into a sibling's subtree and
#: still let the per-format count pass. Both keys stage one ``.deb`` and one
#: ``.rpm``; only the origin attribute tells them apart.
TWO_LINUX_TARGETS = """
[common]
bin_name = "netsuke"

[targets.linux-x86_64]
target = "x86_64-unknown-linux-gnu"
platform = "linux"

[targets.linux-aarch64]
target = "aarch64-unknown-linux-gnu"
platform = "linux"
"""

#: The x86_64 target's archive, kept as a name so a case can refer to it.
X86_ARCHIVE = "netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz"

#: The aarch64 target's archive.
ARM_ARCHIVE = "netsuke-1.0.0-aarch64-unknown-linux-gnu.tar.gz"

#: The aarch64 target's packages, which the x86 record must never claim.
ARM_DEB = "netsuke_1.0.0_arm64.deb"
ARM_RPM = "netsuke-1.0.0-1.aarch64.rpm"

#: Each target, the archive it stages, and the packages it builds.
TWO_LINUX_STAGING = (
    ("linux-x86_64", X86_ARCHIVE, (DEB, RPM)),
    ("linux-aarch64", ARM_ARCHIVE, (ARM_DEB, ARM_RPM)),
)

__all__ = [
    "ARM_ARCHIVE",
    "ARM_DEB",
    "ARM_RPM",
    "TWO_LINUX_STAGING",
    "TWO_LINUX_TARGETS",
    "X86_ARCHIVE",
    "two_target_tree",
]


def two_target_tree(tmp_path: pathlib.Path) -> StagedTree:
    """Stage two Linux producers that each own their own subtree.

    Returns
    -------
    StagedTree
        A complete tree whose two records claim their own archives and
        packages, ready for a case to move one package.

    Notes
    -----
    The packages are staged below each target's own directory because that is
    what :func:`release_package_origins.observe_package_origins` attributes. A
    package at the root would publish the same bytes and carry no origin, so
    a case that moved it could not distinguish a refusal from a missing file.

    Each record describes the bytes actually staged under it, read back off
    disk rather than written as constants: ``require_staged_digests``
    re-hashes the installed payload and compares, so a fabricated digest
    would fail for a reason unrelated to what a case is testing.
    """
    dist = tmp_path / "dist"
    for target, archive, packages in TWO_LINUX_STAGING:
        stage(dist, archive, archive.encode("utf-8"))
        sidecar = f"{archive}.sha256"
        stage(dist, sidecar, sidecar.encode("utf-8"))
        for package in packages:
            stage(dist, f"{target}/{package}", package.encode("utf-8"))
        record_provenance(
            dist,
            fields=Provenance(target=target),
            assets={
                name: release_digest_files.digest_of_file(dist / target / name)
                for name in packages
            }
            | {
                name: release_digest_files.digest_of_file(dist / name)
                for name in (archive, sidecar)
            },
        )
    staging_config = tmp_path / "release-staging.toml"
    _ = staging_config.write_text(TWO_LINUX_TARGETS, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    _ = manifest.write_text(MANIFEST, encoding="utf-8")
    return {
        "dist": dist,
        "staging_config": staging_config,
        "manifest": manifest,
        "output": tmp_path / "artifacts",
        "record": tmp_path / "artifacts" / "trusted-record.json",
    }
