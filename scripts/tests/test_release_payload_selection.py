"""Test the decision that turns three release accounts into one payload.

Each case fixes a specific contradiction between the staging configuration,
the producer records, and the staged names, and asserts that selection refuses
it with the diagnosis that sends an operator to the right file. The
distinction matters more than the refusal: a stale candidate and a missing
target are both "the tree is wrong", and only one of them is fixed by
re-running the failed job.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_payload_selection as selection
from release_payload_errors import (
    PayloadRefusedError,
    TargetPlatformMismatchError,
)
from release_payload_selection_support import (
    ARCHIVE,
    ARM_ARCHIVE,
    ARM_DEB,
    ARM_RPM,
    DEB,
    RPM,
    SIDECAR,
    TARGET,
    TWO_TARGET_ARCHIVES,
    TWO_TARGET_CONFIG,
    arm_record,
    staged_assets,
    two_target_origins,
    two_target_staged,
    x86_record,
)
from release_producer_provenance_errors import (
    DuplicateTargetError,
    ForeignRunError,
    MissingTargetError,
    StaleCandidateError,
    TargetClaimMismatchError,
)

if typ.TYPE_CHECKING:
    from release_producer_provenance import ProducerProvenance

CANDIDATE = "a" * 40
RUN_ID = "1234"

#: The staging configuration selection is judged against, with the target
#: spelled the way the workflow's TOML does.
CONFIG: dict[str, object] = {
    "targets": {TARGET: {"platform": "linux"}},
}


def selection_for(
    provenances: list[ProducerProvenance],
    staged: dict[str, list[str]],
    *,
    archives: dict[str, str] | None = None,
    config: dict[str, object] | None = None,
) -> selection.PayloadSelection[str]:
    """Select a payload from ``staged``, with a caller-chosen source per name.

    Returns
    -------
    selection.PayloadSelection
        The approved selection, whose sources are the literal strings given.
    """
    return selection.select_payload(
        provenances,
        identity=selection.ReleaseIdentity(
            candidate=CANDIDATE,
            run_id=RUN_ID,
            target_config=CONFIG if config is None else config,
        ),
        tree=selection.StagedTree(
            archives={TARGET: ARCHIVE} if archives is None else archives,
            staged=staged,
        ),
    )


def complete_staged() -> dict[str, list[str]]:
    """Return a staged tree that stages one file per declared name."""
    return {ARCHIVE: ["archive"], SIDECAR: ["sidecar"], DEB: ["deb"], RPM: ["rpm"]}


def test_a_complete_tree_selects_exactly_the_declared_names() -> None:
    """Select every declared name once, with its recorded digest."""
    selected = selection_for([x86_record()], complete_staged())
    assert selected.declared == tuple(sorted((ARCHIVE, SIDECAR, DEB, RPM))), (
        "selection must declare every published name"
    )
    assert selected.sources[DEB] == "deb", "the adapter's own source must survive"
    assert selected.trusted[DEB] == "3" * 64, "the recorded digest must survive"


def test_the_sources_are_returned_opaque() -> None:
    """Pass the adapter's source values through without interpreting them.

    Selection never opens a file, so it must not assume a source is a path.
    """
    # One token per key: ``dict.fromkeys`` would evaluate ``object()`` once and
    # hand every key the same object, which would make the identity assertion
    # below pass whatever source the selection returned.
    tokens = {key: object() for key in ("archive", "sidecar", "deb", "rpm")}
    staged = {ARCHIVE: [tokens["archive"]], SIDECAR: [tokens["sidecar"]]}
    staged |= {DEB: [tokens["deb"]], RPM: [tokens["rpm"]]}
    selected = selection.select_payload(
        [x86_record()],
        identity=selection.ReleaseIdentity(CANDIDATE, RUN_ID, CONFIG),
        tree=selection.StagedTree(archives={TARGET: ARCHIVE}, staged=staged),
    )
    assert selected.sources[DEB] is tokens["deb"], (
        "selection must return the adapter's own object, not a copy"
    )


def test_a_record_from_another_commit_is_refused() -> None:
    """Refuse a staged tree left over from an earlier candidate."""
    with pytest.raises(StaleCandidateError):
        selection_for([x86_record(commit="b" * 40)], complete_staged())


def test_a_record_from_another_run_is_refused() -> None:
    """Refuse a record whose producer belongs to a different workflow run."""
    with pytest.raises(ForeignRunError):
        selection_for([x86_record(run_id="9999")], complete_staged())


def test_a_target_without_a_record_is_refused() -> None:
    """Refuse a release that would ship without one configured target."""
    config: dict[str, object] = {
        "targets": {TARGET: {"platform": "linux"}, "macos-arm64": {"platform": "macos"}}
    }
    with pytest.raises(MissingTargetError):
        selection_for([x86_record()], complete_staged(), config=config)


def test_a_duplicated_target_record_is_refused() -> None:
    """Refuse two records claiming the same target.

    Their assets would collide under one published name with different
    digests, and only one of them can be the bytes this release publishes.
    """
    with pytest.raises(DuplicateTargetError):
        selection_for([x86_record(), x86_record()], complete_staged())


def test_a_record_whose_platform_contradicts_the_configuration_is_refused() -> None:
    """Refuse a record built for a platform its target does not serve."""
    with pytest.raises(TargetPlatformMismatchError):
        selection_for([x86_record(platform="macos")], complete_staged())


def test_a_declared_asset_that_was_not_staged_is_refused() -> None:
    """Refuse a tree that omits an asset the records claim."""
    staged = complete_staged()
    del staged[RPM]
    with pytest.raises(PayloadRefusedError, match="was not staged"):
        selection_for([x86_record()], staged)


def test_an_ambiguous_declared_asset_is_refused() -> None:
    """Refuse two staged files claiming one published name."""
    staged = complete_staged()
    staged[RPM] = ["rpm", "another-rpm"]
    with pytest.raises(PayloadRefusedError, match="staged 2 times"):
        selection_for([x86_record()], staged)


def test_a_release_shaped_stray_is_refused() -> None:
    """Refuse an undeclared staged file that would publish if uploaded."""
    staged = complete_staged()
    staged["netsuke-9.9.9-x86_64.tar.gz"] = ["stale"]
    with pytest.raises(PayloadRefusedError, match="not declared"):
        selection_for([x86_record()], staged)


def test_retained_workspace_material_is_excluded() -> None:
    """Exclude build products that ship inside the packages.

    Each of these is present in a real staging tree; none of them is a release
    asset. Refusing them would reject a perfectly good build.
    """
    staged = complete_staged()
    staged |= {"netsuke": ["bin"], "LICENSE": ["licence"], "README.md": ["readme"]}
    selected = selection_for([x86_record()], staged)
    assert "netsuke" not in selected.declared, "the retained binary is not an asset"
    assert "LICENSE" not in selected.declared, "the licence is not an asset"


def test_a_record_claiming_a_name_the_declaration_does_not_is_refused() -> None:
    """Refuse records and configuration that disagree about the asset set.

    The configuration names one archive and the record claims another, so the
    two accounts of the release cannot both be right about what publishes.
    """
    other_archive = "netsuke-1.0.0-aarch64-apple-darwin.tar.gz"
    straying = x86_record(
        assets={
            other_archive: "1" * 64,
            f"{other_archive}.sha256": "2" * 64,
            DEB: "3" * 64,
            RPM: "4" * 64,
        }
    )
    with pytest.raises(TargetClaimMismatchError):
        selection_for([straying], {other_archive: ["archive"]})


def test_two_targets_claiming_their_own_assets_are_accepted() -> None:
    """Accept two same-platform records that each claim their own target's set.

    The control for the swap below: the same two records, correctly bound,
    must select cleanly, so a refusal there can only be about the exchange.
    """
    selected = selection_for(
        [x86_record(), arm_record()],
        two_target_staged(),
        archives=TWO_TARGET_ARCHIVES,
        config=TWO_TARGET_CONFIG,
    )
    assert selected.sources[ARM_ARCHIVE] == "arm-archive", (
        "each target's archive must come from its own record's staging directory"
    )


def test_two_records_exchanging_their_archive_and_sidecar_are_refused() -> None:
    """Refuse two records that swap archives while their union is unchanged.

    This is the case a union comparison cannot see. Both records are for
    Linux, each claims one archive, one sidecar, one ``.deb`` and one
    ``.rpm``, and the union of every claimed name is exactly what the release
    declares -- yet each record now vouches for the other target's archive.
    """
    swapped_x86 = x86_record(
        assets={
            ARM_ARCHIVE: "1" * 64,
            f"{ARM_ARCHIVE}.sha256": "2" * 64,
            DEB: "3" * 64,
            RPM: "4" * 64,
        }
    )
    swapped_arm = arm_record(archive=ARCHIVE, packages=(ARM_DEB, ARM_RPM))
    union = set(swapped_x86.assets) | set(swapped_arm.assets)
    assert union == {*two_target_staged()}, (
        "the case must preserve the union, or it tests the wrong refusal"
    )
    with pytest.raises(TargetClaimMismatchError, match="omits its own"):
        selection_for(
            [swapped_x86, swapped_arm],
            two_target_staged(),
            archives=TWO_TARGET_ARCHIVES,
            config=TWO_TARGET_CONFIG,
        )


def test_a_record_claiming_a_second_archive_is_refused() -> None:
    """Refuse a record claiming two archives, even when the union still fits.

    A record that keeps its own archive *and* adds a sibling target's leaves
    the union complete, so only a per-record check can see it.
    """
    greedy = x86_record(assets=staged_assets() | {ARM_ARCHIVE: "5" * 64})
    with pytest.raises(TargetClaimMismatchError, match="neither its archive"):
        selection_for(
            [greedy, arm_record()],
            two_target_staged(),
            archives=TWO_TARGET_ARCHIVES,
            config=TWO_TARGET_CONFIG,
        )


def test_a_record_claiming_a_sibling_targets_package_is_refused() -> None:
    """Refuse a record whose package claim belongs to its sibling's platform.

    Both targets are Linux, so the package half can only be judged by count
    and format. A record claiming three ``.rpm`` files -- one of them its
    sibling's -- is still refused, which is what a count is for.
    """
    doubled = x86_record(assets=staged_assets() | {ARM_RPM: "5" * 64})
    with pytest.raises(TargetClaimMismatchError, match="must claim 1"):
        selection_for(
            [doubled, arm_record()],
            two_target_staged(),
            archives=TWO_TARGET_ARCHIVES,
            config=TWO_TARGET_CONFIG,
        )


def test_a_one_for_one_package_exchange_is_refused() -> None:
    """Refuse two same-platform records that trade one package each.

    Each record still claims one ``.deb`` and one ``.rpm``, each still claims
    its own archive and sidecar, and the union of everything claimed is
    exactly the release's declaration. Every *count* is satisfied, so this is
    the case a count-based check cannot see.

    Position can. Each producer uploads one workflow artefact and writes its
    record inside it, so the record's own downloaded subtree names the target
    that staged every package beside it. ``package_origins`` carries that
    observation here: the ARM ``.deb`` was found under the ARM subtree, and
    the x86 record claims it. Selection refuses.
    """
    traded_x86 = x86_record(
        assets={
            ARCHIVE: "1" * 64,
            SIDECAR: "2" * 64,
            ARM_DEB: "3" * 64,
            RPM: "4" * 64,
        }
    )
    traded_arm = arm_record(packages=(DEB, ARM_RPM))
    assert set(traded_x86.assets) | set(traded_arm.assets) == {*two_target_staged()}, (
        "the case must preserve the union, or it tests the wrong refusal"
    )
    with pytest.raises(TargetClaimMismatchError, match="staged"):
        selection.select_payload(
            [traded_x86, traded_arm],
            identity=selection.ReleaseIdentity(CANDIDATE, RUN_ID, TWO_TARGET_CONFIG),
            tree=selection.StagedTree(
                archives=TWO_TARGET_ARCHIVES,
                staged=two_target_staged(),
                package_origins=two_target_origins(),
            ),
        )


def test_a_package_bound_to_its_own_target_is_accepted() -> None:
    """Accept the same exchange's positive control: correct attribution.

    The record's own packages are found under its own subtree, so nothing is
    refused. Without this case the refusal above would pass even if the check
    refused every tree that carried an origin at all.
    """
    selected = selection.select_payload(
        [x86_record(), arm_record()],
        identity=selection.ReleaseIdentity(CANDIDATE, RUN_ID, TWO_TARGET_CONFIG),
        tree=selection.StagedTree(
            archives=TWO_TARGET_ARCHIVES,
            staged=two_target_staged(),
            package_origins=two_target_origins(),
        ),
    )
    assert set(selected.declared) == set(two_target_staged()), (
        "a correctly bound tree must still be selected"
    )


def test_require_staged_digests_accepts_the_recorded_bytes() -> None:
    """Accept a payload whose bytes match what its producers recorded."""
    selection.require_staged_digests({DEB: "3" * 64}, trusted={DEB: "3" * 64})


def test_require_staged_digests_refuses_replaced_bytes() -> None:
    """Refuse a payload whose bytes differ from the producers' record."""
    with pytest.raises(PayloadRefusedError, match="did not record"):
        selection.require_staged_digests({DEB: "9" * 64}, trusted={DEB: "3" * 64})


def test_require_staged_digests_refuses_an_unhashed_recorded_asset() -> None:
    """Refuse a trusted name that was never hashed at all.

    Absence is not a match: a caller that skipped a name must not have the
    omission read as agreement.
    """
    with pytest.raises(PayloadRefusedError, match="did not record"):
        selection.require_staged_digests({}, trusted={DEB: "3" * 64})
