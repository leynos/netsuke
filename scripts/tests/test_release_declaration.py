"""Specify the pure release-asset naming and package-count policy."""

import pytest
import release_declaration

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
TARGET_CONFIG = {
    "targets": {
        "linux-x86_64": {"platform": "linux"},
        "linux-aarch64": {"platform": "linux"},
        "windows-x86_64": {"platform": "windows"},
        "windows-aarch64": {"platform": "windows"},
        "macos-x86_64": {"platform": "macos"},
        "macos-aarch64": {"platform": "macos"},
    }
}


def _declare(
    staged_packages: tuple[str, ...] = PACKAGES,
    target_config: object = TARGET_CONFIG,
) -> tuple[str, ...]:
    """Declare the consumer set from typed facts, without reading files."""
    return release_declaration.declared_asset_names(
        archive_names=ARCHIVES,
        target_config=target_config,
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
            {"targets": {"solaris": {"platform": "solaris"}}},
        )


def test_a_configuration_without_targets_is_refused() -> None:
    """Refuse an empty target mapping rather than declare a smaller release."""
    with pytest.raises(release_declaration.MissingTargetsError):
        _declare((), {})


def test_a_non_mapping_configuration_is_refused() -> None:
    """Refuse malformed configuration before counting platform targets."""
    with pytest.raises(
        release_declaration.MalformedTargetError,
        match="not a string-keyed mapping",
    ):
        _declare((), [])


def test_a_target_without_a_platform_is_refused() -> None:
    """Refuse a target the policy cannot count under a package platform."""
    with pytest.raises(release_declaration.MalformedTargetError):
        _declare((), {"targets": {"linux": {}}})


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
