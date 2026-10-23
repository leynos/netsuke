"""Specify the set of asset names a correct release declares.

The declaration has two halves with different sources, and the tests are
grouped to match. The archive half must not depend on the staged tree at all,
because its whole value is fixing names before any package exists; a tree with
no archives must still declare all twelve. The package half must depend on the
tree, because three external actions own the naming, and it is held to a count
derived from configuration so a package that failed to build refuses the
declaration rather than shrinking it.

The refusal rows matter as much as the accept rows. A producer that silently
declared whatever it found would make the verifier's exact-set rule vacuous —
the release would be checked against itself — so each way the tree can
disagree with the configuration is a named row below.
"""

import typing as typ

import pytest
import release_declaration

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

STAGING_CONFIG = """\
[common]
bin_name = "netsuke"

[targets.linux-x86_64]
platform = "linux"
target = "x86_64-unknown-linux-gnu"

[targets.linux-aarch64]
platform = "linux"
target = "aarch64-unknown-linux-gnu"

[targets.windows-x86_64]
platform = "windows"
target = "x86_64-pc-windows-msvc"

[targets.windows-aarch64]
platform = "windows"
target = "aarch64-pc-windows-msvc"

[targets.macos-x86_64]
platform = "macos"
target = "x86_64-apple-darwin"

[targets.macos-aarch64]
platform = "macos"
target = "aarch64-apple-darwin"
"""

MANIFEST = """\
[package]
name = "netsuke-build"
version = "0.1.0-beta4"
"""

VERSION = "0.1.0-beta4"

ARCHIVES = (
    "netsuke-build-0.1.0-beta4-aarch64-apple-darwin.tar.gz",
    "netsuke-build-0.1.0-beta4-aarch64-pc-windows-msvc.tar.gz",
    "netsuke-build-0.1.0-beta4-aarch64-unknown-linux-gnu.tar.gz",
    "netsuke-build-0.1.0-beta4-x86_64-apple-darwin.tar.gz",
    "netsuke-build-0.1.0-beta4-x86_64-pc-windows-msvc.tar.gz",
    "netsuke-build-0.1.0-beta4-x86_64-unknown-linux-gnu.tar.gz",
)

PACKAGES = (
    "netsuke-0.1.0.beta4-1.aarch64.rpm",
    "netsuke-0.1.0.beta4-1.x86_64.rpm",
    "netsuke-0.1.0-beta4-macos-arm64.pkg",
    "netsuke-0.1.0-beta4-macos-x86_64.pkg",
    "netsuke-0.1.0-beta4-x64.msi",
    "netsuke-0.1.0.beta4-1_amd64.deb",
    "netsuke-0.1.0.beta4-1_arm64.deb",
    "netsuke-0.1.0-beta4-arm64.msi",
)

#: The staged spellings the three shared actions produce. The two RPMs and two
#: DEBs still carry the prerelease tilde, which is what the published mapping
#: is for; the PKG and MSI names already match their published form.
STAGED_PACKAGES = (
    "netsuke-0.1.0~beta4-1.aarch64.rpm",
    "netsuke-0.1.0~beta4-1.x86_64.rpm",
    "netsuke-0.1.0-beta4-macos-arm64.pkg",
    "netsuke-0.1.0-beta4-macos-x86_64.pkg",
    "netsuke-0.1.0-beta4-x64.msi",
    "netsuke-0.1.0~beta4-1_amd64.deb",
    "netsuke-0.1.0~beta4-1_arm64.deb",
    "netsuke-0.1.0-beta4-arm64.msi",
)


def _workspace(tmp_path: Path, staging_config: str = STAGING_CONFIG) -> dict[str, Path]:
    """Write the configuration, manifest, and empty dist tree under ``tmp_path``.

    Parameters
    ----------
    tmp_path
        Directory the fixture files are created in.
    staging_config
        Staging configuration to write. The default declares every platform;
        a refusal test passes the malformed configuration it wants refused.

    Returns
    -------
    dict[str, Path]
        Paths keyed by ``staging``, ``manifest``, and ``dist``.
    """
    staging = tmp_path / "release-staging.toml"
    _ = staging.write_text(staging_config, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    _ = manifest.write_text(MANIFEST, encoding="utf-8")
    dist = tmp_path / "dist"
    dist.mkdir()
    return {"staging": staging, "manifest": manifest, "dist": dist}


def _stage_packages(
    dist: Path, names: cabc.Iterable[str], *, nested: bool = True
) -> None:
    """Create package files below ``dist`` in the layout the build produces.

    Parameters
    ----------
    dist
        Release staging root.
    names
        Package file names to create.
    nested
        Whether to place them under a workflow-artefact directory, which is
        how ``actions/download-artifact`` lays the tree out, or directly at
        the root.
    """
    directory = dist / "netsuke_windows_aarch64" if nested else dist
    directory.mkdir(parents=True, exist_ok=True)
    for name in names:
        _ = (directory / name).write_bytes(f"package:{name}".encode())


def _declare(workspace: dict[str, Path]) -> tuple[str, ...]:
    """Declare the asset names for a fully staged fixture workspace."""
    return release_declaration.declared_asset_names(
        workspace["dist"], workspace["staging"], workspace["manifest"], VERSION
    )


def test_a_complete_release_declares_the_consumer_set(tmp_path: Path) -> None:
    """Declare exactly the twenty names a correct release publishes.

    The count is asserted as a set, not a length, so a declaration that named
    the right number of the wrong files fails here rather than passing a count
    check and failing later at upload.
    """
    workspace = _workspace(tmp_path)
    _stage_packages(workspace["dist"], STAGED_PACKAGES)

    declared = _declare(workspace)

    expected = set(ARCHIVES) | {f"{name}.sha256" for name in ARCHIVES} | set(PACKAGES)
    assert set(declared) == expected, (
        f"the declaration must be the flat consumer set, missing "
        f"{sorted(expected - set(declared))} and carrying "
        f"{sorted(set(declared) - expected)}"
    )
    assert len(declared) == 20, (
        f"twenty names make the consumer set, got {len(declared)}"
    )


def test_the_declaration_is_sorted(tmp_path: Path) -> None:
    """Keep the declaration stable, so two runs of one build agree byte for byte."""
    workspace = _workspace(tmp_path)
    _stage_packages(workspace["dist"], STAGED_PACKAGES)

    assert list(_declare(workspace)) == sorted(_declare(workspace)), (
        "the declaration must be sorted, or its file would depend on tree order"
    )


def test_the_archive_half_needs_no_staged_file(tmp_path: Path) -> None:
    """Declare the archives from configuration, not from whatever arrived.

    This is the property that makes the declaration non-vacuous. If archives
    came from the tree, a build that staged no archive would declare none and
    the exact-set check would pass over a release ``cargo binstall`` cannot
    use — the 2026-10-12 defect, one layer up.
    """
    workspace = _workspace(tmp_path)
    _stage_packages(workspace["dist"], ())

    with pytest.raises(release_declaration.MissingPackageError):
        _ = _declare(workspace)


def test_an_empty_tree_still_refuses_on_the_packages(tmp_path: Path) -> None:
    """Fail on the half that is missing, naming the format and the count."""
    workspace = _workspace(tmp_path)

    with pytest.raises(release_declaration.MissingPackageError) as refusal:
        _ = _declare(workspace)

    assert "2" in str(refusal.value), (
        f"the refusal must state the required count, got {refusal.value}"
    )


def test_a_missing_package_refuses_the_declaration(tmp_path: Path) -> None:
    """Refuse rather than shrink when one platform's package failed to build."""
    workspace = _workspace(tmp_path)
    _stage_packages(workspace["dist"], STAGED_PACKAGES[:-1])

    with pytest.raises(release_declaration.MissingPackageError) as refusal:
        _ = _declare(workspace)

    assert ".msi" in str(refusal.value), (
        f"the refusal must name the under-counted format, got {refusal.value}"
    )


def test_a_surplus_package_refuses_the_declaration(tmp_path: Path) -> None:
    """Refuse a format count above the configuration, so a stray file is caught."""
    workspace = _workspace(tmp_path)
    _stage_packages(workspace["dist"], (*STAGED_PACKAGES, "extra.pkg"))

    with pytest.raises(release_declaration.MissingPackageError):
        _ = _declare(workspace)


def test_two_packages_publishing_under_one_name_are_refused(tmp_path: Path) -> None:
    """Refuse a collision, since the declaration would name one asset twice.

    Two staged names differing only in the tilde publish identically, and the
    upload step could then publish either one. The declaration cannot say
    which, so it says so.

    The staged set is built to reach the duplicate check *specifically*. An
    earlier version of this test appended a third ``.deb`` to the full fixture,
    so the format count refused the tree first and
    :class:`release_declaration.DuplicatePackageError` was never raised; the
    test passed without exercising the rule its name claims. Two ``.deb`` files
    that publish under one name satisfy the count and the collision at once, so
    the refusal below can only come from the duplicate rule.
    """
    workspace = _workspace(tmp_path)
    retired = "netsuke-0.1.0~beta4-1_arm64.deb"
    colliding = (
        *(name for name in STAGED_PACKAGES if name != retired),
        "netsuke-0.1.0.beta4-1_amd64.deb",
    )
    _stage_packages(workspace["dist"], colliding)

    with pytest.raises(release_declaration.DuplicatePackageError) as refusal:
        _ = _declare(workspace)

    assert "netsuke-0.1.0.beta4-1_amd64.deb" in str(refusal.value), (
        f"the refusal must name the colliding asset, got {refusal.value}"
    )


def test_a_platform_with_no_declared_format_is_refused(tmp_path: Path) -> None:
    """Refuse a configured platform this build stages no package format for.

    Silently ignoring it would declare a release missing that platform's
    packages, which is the failure the count is meant to prevent.
    """
    workspace = _workspace(
        tmp_path,
        '[targets.a]\nplatform = "solaris"\ntarget = "sparc-unknown-solaris"\n',
    )

    with pytest.raises(release_declaration.UnknownPlatformError):
        _ = _declare(workspace)


def test_a_configuration_without_targets_is_refused(tmp_path: Path) -> None:
    """Refuse a configuration that declares no target at all."""
    workspace = _workspace(tmp_path, "[common]\nbin_name = 'netsuke'\n")

    with pytest.raises(release_declaration.MissingTargetsError):
        _ = _declare(workspace)


def test_a_target_without_a_platform_is_refused(tmp_path: Path) -> None:
    """Refuse an uncountable target rather than counting it as its own platform."""
    workspace = _workspace(
        tmp_path, '[targets.a]\ntarget = "x86_64-unknown-linux-gnu"\n'
    )

    with pytest.raises(release_declaration.MalformedTargetError):
        _ = _declare(workspace)


@pytest.mark.parametrize(
    ("staged", "published"),
    [
        ("netsuke-0.1.0~beta4-1.x86_64.rpm", "netsuke-0.1.0.beta4-1.x86_64.rpm"),
        ("netsuke_0.1.0~beta4-1_arm64.deb", "netsuke_0.1.0.beta4-1_arm64.deb"),
        ("netsuke-0.1.0-beta4-macos-arm64.pkg", "netsuke-0.1.0-beta4-macos-arm64.pkg"),
        ("netsuke-0.1.0-beta4-x64.msi", "netsuke-0.1.0-beta4-x64.msi"),
    ],
)
def test_the_published_name_maps_only_the_characters_github_rewrites(
    staged: str, published: str
) -> None:
    """Map the tilde and leave every other character alone."""
    assert release_declaration.published_name(staged) == published, (
        "the mapping must apply to the tilde alone, not sanitise freely"
    )


def test_the_packages_come_from_the_staged_tree(tmp_path: Path) -> None:
    """Read package names from the tree, since external actions own their shape.

    A renamed package must therefore appear in the declaration as it was
    staged, rather than as any rule in this repository predicts.
    """
    workspace = _workspace(tmp_path)
    renamed = (*STAGED_PACKAGES[:-1], "renamed-by-the-action.msi")
    _stage_packages(workspace["dist"], renamed)

    declared = _declare(workspace)

    assert "renamed-by-the-action.msi" in declared, (
        "a package's name must be taken from the tree, not reimplemented"
    )
    assert "netsuke-0.1.0-beta4-arm64.msi" not in declared, (
        "a name no file carries must never be declared"
    )


def test_packages_at_the_dist_root_are_found(tmp_path: Path) -> None:
    """Find packages wherever the download put them, not only under a nesting."""
    workspace = _workspace(tmp_path)
    _stage_packages(workspace["dist"], STAGED_PACKAGES, nested=False)

    declared = _declare(workspace)

    assert set(PACKAGES) <= set(declared), (
        "a package at the release root is as publishable as a nested one"
    )
