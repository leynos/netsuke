"""Specify the staging-configuration decoder that feeds release policy.

:func:`release_declaration_cli.staging_targets` is the boundary between parsed
TOML and the typed :class:`release_declaration.StagingTargets` the declaration
policy decides over. The malformed-configuration suite drives that decoder
through the CLI with real TOML, which cannot express a non-mapping
configuration or a non-string-keyed mapping: those branches are reachable only
by calling the decoder directly, so they are exercised here.

The decode is also checked end to end, because a decoder that produced the
right platforms in the wrong multiplicity would still satisfy a per-case test
while declaring the wrong packages.
"""

import pytest
import release_declaration
import release_package_formats
from release_declaration_cli import staging_targets

#: A staging configuration with repeated and distinct platforms, in order.
CONFIG: dict[str, object] = {
    "targets": {
        "linux-x86_64": {"platform": "linux"},
        "windows-x64": {"platform": "windows"},
        "linux-aarch64": {"platform": "linux"},
    },
}

#: Package names that satisfy one `.deb`, `.rpm`, and `.msi` per target.
STAGED_PACKAGES = (
    "crate_aarch64.deb",
    "crate_x86_64.deb",
    "crate.aarch64.rpm",
    "crate.x86_64.rpm",
    "crate.x64.msi",
)


@pytest.fixture(name="decoded")
def decoded_fixture() -> release_declaration.StagingTargets:
    """Return the decoded staging targets for :data:`CONFIG`."""
    return staging_targets(CONFIG)


def test_the_decode_keeps_every_target_in_configuration_order(
    decoded: release_declaration.StagingTargets,
) -> None:
    """Preserve order and repetition, so a repeated platform counts twice.

    The policy counts targets rather than platforms, so collapsing the two
    ``linux`` entries into one would under-declare the release's packages.
    """
    platforms = tuple(target.platform for target in decoded.targets)

    assert platforms == ("linux", "windows", "linux"), (
        f"the decode must keep every configured target, got {platforms}"
    )


def test_the_decoded_targets_count_by_platform(
    decoded: release_declaration.StagingTargets,
) -> None:
    """Count repeated platforms, which drives the per-platform package count."""
    counts = release_declaration.platform_target_counts(decoded)

    assert counts == {"linux": 2, "windows": 1}, (
        f"each configured target must be counted once, got {counts}"
    )


def test_a_decoded_configuration_declares_the_exact_asset_set(
    decoded: release_declaration.StagingTargets,
) -> None:
    """Declare the sorted archive, sidecar, and per-target package names.

    Two Linux targets need two packages per Linux format, so the exact
    declaration is the check that the decode's multiplicity reached policy.
    """
    declared = release_declaration.declared_asset_names(
        archive_names=["crate.tar.gz"],
        targets=decoded,
        staged_package_names=STAGED_PACKAGES,
    )

    expected = (
        "crate.aarch64.rpm",
        "crate.tar.gz",
        "crate.tar.gz.sha256",
        "crate.x64.msi",
        "crate.x86_64.rpm",
        "crate_aarch64.deb",
        "crate_x86_64.deb",
    )
    assert declared == expected, f"the declaration must be exact, got {declared}"


def test_a_malformed_later_target_is_refused() -> None:
    """Refuse a bad target even when the earlier ones decoded cleanly.

    The decoder reads each target as it yields it, so a refusal must not
    depend on the malformed entry arriving first.
    """
    config: dict[str, object] = {
        "targets": {
            "linux-x86_64": {"platform": "linux"},
            "windows-x64": {},
        }
    }

    with pytest.raises(release_package_formats.MalformedTargetError) as raised:
        staging_targets(config)

    assert str(raised.value) == (
        "staging configuration has a target without a platform"
    ), f"the later target's own diagnosis must reach the caller, got {raised.value}"


@pytest.mark.parametrize(
    "config",
    [
        pytest.param([], id="list"),
        pytest.param("targets", id="string"),
        pytest.param(None, id="none"),
        pytest.param(1, id="integer"),
        pytest.param({1: {"targets": {}}}, id="non-string-key"),
    ],
)
def test_a_configuration_that_is_not_a_string_keyed_mapping_is_refused(
    config: object,
) -> None:
    """Refuse a shape the policy could not read, with the shared diagnosis.

    TOML always decodes to a string-keyed table, so only a direct call can
    reach these branches; the point of the test is that a caller which does
    reach one still gets a configuration diagnosis rather than a ``TypeError``.
    """
    with pytest.raises(release_package_formats.MalformedTargetError) as raised:
        staging_targets(config)

    assert str(raised.value) == (
        "staging configuration is not a string-keyed mapping"
    ), f"a non-mapping configuration must be diagnosed, got {raised.value}"


def test_an_absent_target_table_is_refused() -> None:
    """Refuse a mapping that carries no ``targets`` table at all.

    A ``targets`` key holding a non-table value is the other way to reach this
    refusal, and the malformed-configuration suite pins those shapes through
    real TOML; this case is the absent key that only a direct call expresses
    unambiguously.
    """
    with pytest.raises(release_package_formats.MissingTargetsError):
        staging_targets({})


def test_an_empty_target_table_is_refused() -> None:
    """Refuse a ``targets`` table with no entry, which would publish nothing."""
    with pytest.raises(release_package_formats.MissingTargetsError):
        staging_targets({"targets": {}})


def test_the_decoder_accepts_a_typed_configuration() -> None:
    """Decode a single well-formed target, pinning the happy path.

    The remaining cases describe refusals; this one is the baseline that they
    are departures from.
    """
    decoded = staging_targets({"targets": {"macos-arm64": {"platform": "macos"}}})

    assert decoded == release_declaration.StagingTargets((
        release_declaration.StagingTarget("macos"),
    )), "one configured target must decode to one staging target"
