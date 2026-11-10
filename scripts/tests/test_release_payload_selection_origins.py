"""Test the package-origin half of selection: who staged what, and where.

A package name cannot be derived from the archive or from the configuration.
Three external shared actions own those names, so a per-format count is all the
name itself supports. Position is the second account: each producer uploads one
workflow artefact, the release downloads each into its own subtree of ``dist``,
and every package found beside a record is attributed to the target that record
names.

Selection meets that observation in three shapes and must not confuse them. A
mapping with an entry per claimed package is the walked tree; ``None`` is a
caller that did not walk; ``{}`` is a walk that attributed nothing. The cases
here state what each shape admits, and the two-count-and-one-count-satisfied
exchange a per-format count cannot see.

Run via ``make test-release-publication``.
"""

import pytest
import release_payload_selection as selection
from release_payload_selection_support import (
    ARCHIVE,
    ARM_DEB,
    ARM_RPM,
    CANDIDATE,
    DEB,
    RPM,
    RUN_ID,
    SIDECAR,
    TWO_TARGET_ARCHIVES,
    TWO_TARGET_CONFIG,
    arm_record,
    two_target_origins,
    two_target_staged,
    x86_record,
)
from release_producer_provenance_errors import TargetClaimMismatchError


def _identity() -> selection.ReleaseIdentity:
    """Return the two-target identity every case here is judged against.

    Returns
    -------
    selection.ReleaseIdentity
        The candidate, run, and configuration both records must serve.
    """
    return selection.ReleaseIdentity(CANDIDATE, RUN_ID, TWO_TARGET_CONFIG)


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
            identity=_identity(),
            tree=selection.StagedTree(
                archives=TWO_TARGET_ARCHIVES,
                staged=two_target_staged(),
                package_origins=two_target_origins(),
            ),
        )


def test_a_package_no_subtree_staged_is_refused() -> None:
    """Refuse a claimed package the walk could not attribute to any target.

    A package the download left at the ``dist`` root, or in a subtree holding
    no producer record, keeps its published name. The staged-name check
    therefore matches it, and its digest can still agree with the record, so
    nothing else in selection notices. Origins do: the adapter found no
    record beside it, so no target owns it.
    """
    origins = two_target_origins()
    del origins[DEB]
    with pytest.raises(TargetClaimMismatchError, match="no producer subtree"):
        selection.select_payload(
            [x86_record(), arm_record()],
            identity=_identity(),
            tree=selection.StagedTree(
                archives=TWO_TARGET_ARCHIVES,
                staged=two_target_staged(),
                package_origins=origins,
            ),
        )


def test_an_unobserved_tree_is_judged_by_its_counts_alone() -> None:
    """Accept a tree whose origins were never observed.

    ``package_origins=None`` says the caller did not look, not that it looked
    and found nothing. Refusing here would make every caller without a walk
    fail, so the count checks are the whole package guarantee in that case.
    """
    selected = selection.select_payload(
        [x86_record(), arm_record()],
        identity=_identity(),
        tree=selection.StagedTree(
            archives=TWO_TARGET_ARCHIVES,
            staged=two_target_staged(),
            package_origins=None,
        ),
    )
    assert set(selected.declared) == set(two_target_staged()), (
        "an unobserved tree must still select when its counts hold"
    )


def test_an_observed_tree_that_attributed_nothing_is_refused() -> None:
    """Refuse every claimed package when the walk attributed none.

    The empty mapping is the case a falsy default would swallow: it says the
    tree *was* walked, and no package was found beside any producer record.
    """
    with pytest.raises(TargetClaimMismatchError, match="no producer subtree"):
        selection.select_payload(
            [x86_record(), arm_record()],
            identity=_identity(),
            tree=selection.StagedTree(
                archives=TWO_TARGET_ARCHIVES,
                staged=two_target_staged(),
                package_origins={},
            ),
        )


def test_a_package_bound_to_its_own_target_is_accepted() -> None:
    """Accept the exchange's positive control: correct attribution.

    The record's own packages are found under its own subtree, so nothing is
    refused. Without this case the refusal above would pass even if the check
    refused every tree that carried an origin at all.
    """
    selected = selection.select_payload(
        [x86_record(), arm_record()],
        identity=_identity(),
        tree=selection.StagedTree(
            archives=TWO_TARGET_ARCHIVES,
            staged=two_target_staged(),
            package_origins=two_target_origins(),
        ),
    )
    assert set(selected.declared) == set(two_target_staged()), (
        "a correctly bound tree must still be selected"
    )
