"""Generated properties for the selection that binds records to a payload.

The examples in ``test_release_payload_selection.py`` state one contradiction
at a time. These cases state the *relations* that must hold across every
well-formed input, whatever its shape: selection must declare exactly the
names its records claim, must carry each name's recorded digest through
unchanged, must return each name's own staged source, and must answer the same
way to a reordering of the records or of the staged mapping.

The refusals are generated too, because a property about acceptance alone
would hold for a function that accepted anything. A tree missing a declared
name and a tree staging one twice are each generated from a valid tree and
must both be refused.

Run via ``make test-release-publication``.
"""

import dataclasses
import typing as typ

import pytest
import release_payload_generation_support as generated
import release_payload_selection as selection
from hypothesis import HealthCheck, given, settings
from release_payload_errors import PayloadRefusedError
from release_payload_generation_support import (
    CANDIDATE,
    DEB_SUFFIX,
    RUN_ID,
)
from release_producer_provenance_errors import (
    DuplicateAssetError,
    DuplicateTargetError,
    TargetClaimMismatchError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_producer_provenance import ProducerProvenance


def _select(
    records: cabc.Sequence[ProducerProvenance],
    staged: dict[str, list[str]],
    pairs: cabc.Sequence[tuple[str, str]],
    *,
    package_origins: cabc.Mapping[str, str] | None = None,
) -> selection.PayloadSelection[str]:
    """Select from ``staged`` against the configuration ``pairs`` describes.

    Returns
    -------
    selection.PayloadSelection
        The approved selection.
    """
    return selection.select_payload(
        records,
        identity=selection.ReleaseIdentity(
            candidate=CANDIDATE,
            run_id=RUN_ID,
            target_config={
                "targets": {target: {"platform": "linux"} for target, _ in pairs}
            },
        ),
        tree=selection.StagedTree(
            archives=dict(pairs),
            staged=staged,
            package_origins=package_origins,
        ),
    )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set())
def test_selection_declares_exactly_the_claimed_names(
    pairs: list[tuple[str, str]],
) -> None:
    """Declare every claimed name once, whatever the target set looks like."""
    claimed = generated.claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    selected = _select(
        [record for record, _assets in claimed], generated.staged_for(every_name), pairs
    )
    assert set(selected.declared) == every_name, (
        "selection must declare exactly the names its records claim"
    )
    assert len(selected.declared) == len(every_name), (
        "no declared name may appear twice"
    )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set())
def test_selection_carries_each_recorded_digest_unchanged(
    pairs: list[tuple[str, str]],
) -> None:
    """Carry each claimed name's digest through without substitution."""
    claimed = generated.claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    selected = _select(
        [record for record, _assets in claimed], generated.staged_for(every_name), pairs
    )
    for _record, assets in claimed:
        for name, digest in assets.items():
            assert selected.trusted[name] == digest, (
                f"{name} must keep the digest its producer recorded"
            )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set())
def test_selection_returns_each_names_own_source(
    pairs: list[tuple[str, str]],
) -> None:
    """Return the source the adapter observed for that name, not a sibling's."""
    claimed = generated.claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    staged = generated.staged_for(every_name)
    selected = _select([record for record, _assets in claimed], staged, pairs)
    for name in every_name:
        assert selected.sources[name] == staged[name][0], (
            f"{name} must keep the source staged under its own name"
        )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set())
def test_selection_is_indifferent_to_record_and_staging_order(
    pairs: list[tuple[str, str]],
) -> None:
    """Answer identically however the records and the staged mapping are ordered.

    The staged mapping is rebuilt in reverse and the records are reversed, so
    any answer that depended on insertion order would differ.
    """
    claimed = generated.claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    forward = _select(
        [record for record, _assets in claimed], generated.staged_for(every_name), pairs
    )
    backward = _select(
        [record for record, _assets in reversed(claimed)],
        dict(reversed(list(generated.staged_for(every_name).items()))),
        list(reversed(pairs)),
    )
    assert forward.declared == backward.declared, (
        "the declared names must not depend on input order"
    )
    assert forward.trusted == backward.trusted, (
        "the trusted digests must not depend on input order"
    )
    assert forward.sources == backward.sources, (
        "the sources must not depend on input order"
    )


@settings(
    max_examples=50,
    deadline=None,
    derandomize=True,
    suppress_health_check=[HealthCheck.function_scoped_fixture],
)
@given(pairs=generated.target_set())
def test_every_declared_name_is_refused_when_omitted(
    pairs: list[tuple[str, str]],
) -> None:
    """Refuse a staged tree that omits any one of its declared names.

    Every name in the generated tree is damaged in turn rather than one drawn
    index. An index bounded by a constant only ever reached the names sorted
    first -- a tree may hold twelve -- so the property held for whichever names
    the generator happened to produce early and for none of the rest.
    """
    records, names = generated.valid_tree(pairs)
    for omitted in names:
        staged = generated.staged_for(names)
        del staged[omitted]
        with pytest.raises(PayloadRefusedError, match="was not staged"):
            _select(records, staged, pairs)


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set())
def test_every_declared_name_is_refused_when_doubly_staged(
    pairs: list[tuple[str, str]],
) -> None:
    """Refuse a staged tree that stages any one declared name twice.

    Two sources claim one published name and nothing says which is the
    release's, so neither may be chosen. As with the omission case, every name
    is doubled in turn so no generated name goes unchecked.
    """
    records, names = generated.valid_tree(pairs)
    for doubled in names:
        staged = generated.staged_for(names)
        staged[doubled] = [*staged[doubled], f"duplicate:{doubled}"]
        with pytest.raises(PayloadRefusedError, match="was staged 2 times"):
            _select(records, staged, pairs)


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set())
def test_a_duplicated_target_is_refused(pairs: list[tuple[str, str]]) -> None:
    """Refuse two records that claim the same target."""
    claimed = generated.claims(pairs)
    records = [record for record, _assets in claimed]
    every_name = {name for _record, assets in claimed for name in assets}
    with pytest.raises(DuplicateTargetError):
        _select([*records, records[0]], generated.staged_for(every_name), pairs)


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set(min_targets=2, max_targets=2))
def test_a_package_name_two_targets_claim_is_refused(
    pairs: list[tuple[str, str]],
) -> None:
    """Refuse two configured targets claiming one package name.

    The second record trades its own ``.deb`` for the first target's. Every
    per-format count still reads one, and both archives stay with their own
    targets, so the per-target check passes and only the cross-record claim is
    wrong. That is what makes this the case for the union rather than for the
    binding.

    The strategy draws exactly two targets, because a cross-target clash needs
    two and the case rewrites the second record's claims in terms of the
    first's. Drawing a third would leave that record claiming none of its own
    packages, which the per-target check refuses before the cross-record claim
    is ever reached.
    """
    claimed = generated.claims(pairs)
    (first_record, first_assets), (second_record, second_assets) = (
        claimed[0],
        claimed[1],
    )
    shared = next(name for name in first_assets if name.endswith(DEB_SUFFIX))
    # Keep the second record's archive, sidecar, and `.rpm`; give up its own
    # `.deb` for the first target's. Counts stay at one per format.
    traded = {
        name: digest
        for name, digest in second_assets.items()
        if not name.endswith(DEB_SUFFIX)
    }
    traded[shared] = "f" * 64
    every_name = {*first_assets, *traded}
    with pytest.raises(DuplicateAssetError):
        _select(
            [
                first_record,
                dataclasses.replace(second_record, assets=traded),
            ],
            generated.staged_for(every_name),
            pairs,
        )


def _exchange_debs(
    first: tuple[ProducerProvenance, dict[str, str]],
    second: tuple[ProducerProvenance, dict[str, str]],
) -> list[ProducerProvenance]:
    """Rewrite two records so each claims the other target's ``.deb``.

    Each record gives up its own ``.deb`` and takes the sibling's. The union
    of claimed names is unchanged, each record still claims exactly one
    package per format, and its archive and sidecar stay its own -- so a count
    check sees nothing wrong and only the downloaded position differs.

    Returns
    -------
    list[ProducerProvenance]
        The two rewritten records, in the order given.
    """
    (first_record, first_assets), (second_record, second_assets) = first, second
    first_deb = next(name for name in first_assets if name.endswith(DEB_SUFFIX))
    second_deb = next(name for name in second_assets if name.endswith(DEB_SUFFIX))
    traded_first = {
        name: digest for name, digest in first_assets.items() if name != first_deb
    } | {second_deb: "f" * 64}
    traded_second = {
        name: digest for name, digest in second_assets.items() if name != second_deb
    } | {first_deb: "e" * 64}
    return [
        dataclasses.replace(first_record, assets=traded_first),
        dataclasses.replace(second_record, assets=traded_second),
    ]


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=generated.target_set(min_targets=2, max_targets=2))
def test_a_package_exchange_between_targets_is_refused_by_position(
    pairs: list[tuple[str, str]],
) -> None:
    """Refuse a same-format package exchange at one-for-one parity.

    Unlike the union case above, the two records keep the *union* intact: each
    still claims exactly one ``.deb`` and one ``.rpm``, so every count is
    satisfied and no name is claimed twice. Only the downloaded position
    distinguishes them, and ``package_origins`` is that observation.

    Both records are rewritten so each claims the other's ``.deb``, which is
    the exact trade a count cannot see. The origins stay as the tree reported
    them -- each target's own ``.deb`` under its own subtree -- so the
    refusal is attributable to the trade rather than to a missing observation.
    """
    claimed = generated.claims(pairs)
    records = _exchange_debs(claimed[0], claimed[1])
    origins = generated.origins(claimed)
    first_deb = next(name for name in claimed[0][1] if name.endswith(DEB_SUFFIX))
    assert origins[first_deb] == records[0].target, (
        "the observation must attribute each package to its own target"
    )
    every_name = {name for record in records for name in record.assets}
    with pytest.raises(TargetClaimMismatchError, match="staged"):
        _select(
            records, generated.staged_for(every_name), pairs, package_origins=origins
        )
