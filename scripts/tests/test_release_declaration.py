"""Specify the pure release-asset naming and package-count policy."""

import typing as typ

import pytest
import release_declaration

if typ.TYPE_CHECKING:
    import collections.abc as cabc

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
STAGING_TARGETS = release_declaration.StagingTargets(
    tuple(
        release_declaration.StagingTarget(platform)
        for platform in ("linux", "linux", "windows", "windows", "macos", "macos")
    )
)


def _declare(
    staged_packages: tuple[str, ...] = PACKAGES,
    targets: release_declaration.StagingTargets = STAGING_TARGETS,
) -> tuple[str, ...]:
    """Declare the consumer set from typed facts, without reading files."""
    return release_declaration.declared_asset_names(
        archive_names=ARCHIVES,
        targets=targets,
        staged_package_names=staged_packages,
    )


def test_a_complete_release_declares_the_consumer_set() -> None:
    """Declare exactly the twenty assets a correct release publishes."""
    declared = _declare()
    expected = set(ARCHIVES) | {f"{name}.sha256" for name in ARCHIVES} | set(PACKAGES)

    assert set(declared) == expected, (
        "the declaration must contain exactly the expected assets"
    )
    assert len(declared) == 20, "the current release declaration has twenty assets"


def test_the_declaration_is_sorted() -> None:
    """Keep the returned asset names stable across runs."""
    assert list(_declare()) == sorted(_declare()), (
        "declared asset names must have a stable sorted order"
    )


def test_each_configured_archive_gets_a_sidecar() -> None:
    """Pair adapter-provided archive names with their sidecar names."""
    paired = release_declaration.archive_asset_names(("one.tar.gz", "two.tar.gz"))

    assert paired == (
        "one.tar.gz",
        "one.tar.gz.sha256",
        "two.tar.gz",
        "two.tar.gz.sha256",
    ), "each archive must be followed by its matching sidecar"


def test_a_missing_package_refuses_instead_of_shrinking_the_declaration() -> None:
    """Refuse when a configured target's format count is incomplete."""
    with pytest.raises(release_declaration.MissingPackageError, match=r"\.msi"):
        _declare(PACKAGES[:-1])


def test_a_surplus_package_refuses_the_declaration() -> None:
    """Refuse a format count above its configured target count."""
    with pytest.raises(release_declaration.MissingPackageError):
        _declare((*PACKAGES, "extra.pkg"))


def test_two_packages_publishing_under_one_name_are_refused() -> None:
    """Reject two observed package names that map to the same public name."""
    replaced = (
        *(name for name in PACKAGES if not name.endswith("_arm64.deb")),
        "netsuke-0.1.0.beta4-1_amd64.deb",
    )

    with pytest.raises(release_declaration.DuplicatePackageError):
        _declare(replaced)


def test_a_platform_with_no_declared_format_is_refused() -> None:
    """Refuse a configured platform the package policy cannot represent."""
    with pytest.raises(release_declaration.UnknownPlatformError):
        _declare(
            (),
            release_declaration.StagingTargets((
                release_declaration.StagingTarget("solaris"),
            )),
        )


@pytest.mark.parametrize(
    "build",
    [
        pytest.param(release_declaration.StagingTargets, id="direct-construction"),
        pytest.param(release_declaration.StagingTargets.from_targets, id="factory"),
    ],
)
def test_a_configuration_without_targets_is_refused(
    build: cabc.Callable[
        [tuple[release_declaration.StagingTarget, ...]],
        release_declaration.StagingTargets,
    ],
) -> None:
    """Refuse an empty target set rather than declare a smaller release.

    Both construction paths are exercised: the invariant belongs to the
    record, so a caller that skips the factory must not obtain a record the
    policy would then under-declare from.
    """
    with pytest.raises(release_declaration.MissingTargetsError):
        _ = build(())


@pytest.mark.parametrize(
    "platform",
    [
        pytest.param(None, id="absent"),
        pytest.param(1, id="integer"),
        pytest.param("", id="empty"),
    ],
)
def test_a_target_without_a_platform_string_is_refused(platform: object) -> None:
    """Refuse a target whose platform cannot key a package-format table.

    Counting and sorting the configured platforms would compare a mixed-type
    key set and raise ``TypeError``, so the target refuses the value itself.
    """
    malformed = typ.cast("str", platform)

    with pytest.raises(release_declaration.MalformedTargetError):
        _ = release_declaration.StagingTarget(malformed)


def test_package_names_are_observed_from_the_supplied_tree_facts() -> None:
    """Carry an action-provided package name without predicting its spelling."""
    renamed = (*PACKAGES[:-1], "renamed-by-the-action.msi")

    declared = _declare(renamed)

    assert "renamed-by-the-action.msi" in declared, (
        "the action-provided package name must be retained"
    )
    assert "netsuke-0.1.0-beta4-arm64.msi" not in declared, (
        "the declaration must not predict an asset name the action did not provide"
    )
