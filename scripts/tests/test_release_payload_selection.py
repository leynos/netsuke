"""Test the decision that turns three release accounts into one payload.

Each case fixes a specific contradiction between the staging configuration,
the producer records, and the staged names, and asserts that selection refuses
it with the diagnosis that sends an operator to the right file. The
distinction matters more than the refusal: a stale candidate and a missing
target are both "the tree is wrong", and only one of them is fixed by
re-running the failed job.

Run via ``make test-release-publication``.
"""

import pytest
import release_payload_selection as selection
from release_payload_errors import (
    ClaimedDeclarationMismatchError,
    PayloadRefusedError,
    TargetPlatformMismatchError,
)
from release_producer_provenance import ProducerProvenance
from release_producer_provenance_errors import (
    DuplicateTargetError,
    ForeignRunError,
    MissingTargetError,
    StaleCandidateError,
)

CANDIDATE = "a" * 40
RUN_ID = "1234"
TARGET = "linux-x86_64"

#: The staging configuration selection is judged against, with the target
#: spelled the way the workflow's TOML does.
CONFIG: dict[str, object] = {
    "targets": {TARGET: {"platform": "linux"}},
}

ARCHIVE = "netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
DEB = "netsuke_1.0.0_amd64.deb"
RPM = "netsuke-1.0.0-1.x86_64.rpm"


def provenance(**overrides: object) -> ProducerProvenance:
    """Build a producer record for the candidate, overridden field by field.

    Returns
    -------
    ProducerProvenance
        A record claiming the archive, its sidecar, and two packages.
    """
    fields: dict[str, object] = {
        "schema": 1,
        "commit": CANDIDATE,
        "run_id": RUN_ID,
        "run_attempt": "1",
        "target": TARGET,
        "platform": "linux",
        "assets": {
            ARCHIVE: "1" * 64,
            SIDECAR: "2" * 64,
            DEB: "3" * 64,
            RPM: "4" * 64,
        },
    }
    fields.update(overrides)
    return ProducerProvenance(**fields)


def selection_for(
    provenances: list[ProducerProvenance],
    staged: dict[str, list[str]],
    *,
    archive_names: list[str] | None = None,
    config: dict[str, object] = CONFIG,
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
            candidate=CANDIDATE, run_id=RUN_ID, target_config=config
        ),
        archive_names=[ARCHIVE] if archive_names is None else archive_names,
        staged=staged,
    )


def complete_staged() -> dict[str, list[str]]:
    """Return a staged tree that stages one file per declared name."""
    return {ARCHIVE: ["archive"], SIDECAR: ["sidecar"], DEB: ["deb"], RPM: ["rpm"]}


def test_a_complete_tree_selects_exactly_the_declared_names() -> None:
    """Select every declared name once, with its recorded digest."""
    selected = selection_for([provenance()], complete_staged())
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
        [provenance()],
        identity=selection.ReleaseIdentity(CANDIDATE, RUN_ID, CONFIG),
        archive_names=[ARCHIVE],
        staged=staged,
    )
    assert selected.sources[DEB] is tokens["deb"], (
        "selection must return the adapter's own object, not a copy"
    )


def test_a_record_from_another_commit_is_refused() -> None:
    """Refuse a staged tree left over from an earlier candidate."""
    with pytest.raises(StaleCandidateError):
        selection_for([provenance(commit="b" * 40)], complete_staged())


def test_a_record_from_another_run_is_refused() -> None:
    """Refuse a record whose producer belongs to a different workflow run."""
    with pytest.raises(ForeignRunError):
        selection_for([provenance(run_id="9999")], complete_staged())


def test_a_target_without_a_record_is_refused() -> None:
    """Refuse a release that would ship without one configured target."""
    config = {
        "targets": {TARGET: {"platform": "linux"}, "macos-arm64": {"platform": "macos"}}
    }
    with pytest.raises(MissingTargetError):
        selection_for([provenance()], complete_staged(), config=config)


def test_a_duplicated_target_record_is_refused() -> None:
    """Refuse two records claiming the same target.

    Their assets would collide under one published name with different
    digests, and only one of them can be the bytes this release publishes.
    """
    with pytest.raises(DuplicateTargetError):
        selection_for([provenance(), provenance()], complete_staged())


def test_a_record_whose_platform_contradicts_the_configuration_is_refused() -> None:
    """Refuse a record built for a platform its target does not serve."""
    with pytest.raises(TargetPlatformMismatchError):
        selection_for([provenance(platform="macos")], complete_staged())


def test_a_declared_asset_that_was_not_staged_is_refused() -> None:
    """Refuse a tree that omits an asset the records claim."""
    staged = complete_staged()
    del staged[RPM]
    with pytest.raises(PayloadRefusedError, match="was not staged"):
        selection_for([provenance()], staged)


def test_an_ambiguous_declared_asset_is_refused() -> None:
    """Refuse two staged files claiming one published name."""
    staged = complete_staged()
    staged[RPM] = ["rpm", "another-rpm"]
    with pytest.raises(PayloadRefusedError, match="staged 2 times"):
        selection_for([provenance()], staged)


def test_a_release_shaped_stray_is_refused() -> None:
    """Refuse an undeclared staged file that would publish if uploaded."""
    staged = complete_staged()
    staged["netsuke-9.9.9-x86_64.tar.gz"] = ["stale"]
    with pytest.raises(PayloadRefusedError, match="not declared"):
        selection_for([provenance()], staged)


def test_retained_workspace_material_is_excluded() -> None:
    """Exclude build products that ship inside the packages.

    Each of these is present in a real staging tree; none of them is a release
    asset. Refusing them would reject a perfectly good build.
    """
    staged = complete_staged()
    staged |= {"netsuke": ["bin"], "LICENSE": ["licence"], "README.md": ["readme"]}
    selected = selection_for([provenance()], staged)
    assert "netsuke" not in selected.declared, "the retained binary is not an asset"
    assert "LICENSE" not in selected.declared, "the licence is not an asset"


def test_a_record_claiming_a_name_the_declaration_does_not_is_refused() -> None:
    """Refuse records and configuration that disagree about the asset set.

    The configuration names one archive and the record claims another, so the
    two accounts of the release cannot both be right about what publishes.
    """
    other_archive = "netsuke-1.0.0-aarch64-apple-darwin.tar.gz"
    straying = provenance(
        assets={
            other_archive: "1" * 64,
            f"{other_archive}.sha256": "2" * 64,
            DEB: "3" * 64,
            RPM: "4" * 64,
        }
    )
    with pytest.raises(ClaimedDeclarationMismatchError):
        selection_for([straying], {other_archive: ["archive"]})


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
