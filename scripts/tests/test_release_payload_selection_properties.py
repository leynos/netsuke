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
import release_payload_selection as selection
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st
from release_payload_errors import PayloadRefusedError
from release_producer_provenance import ProducerProvenance
from release_producer_provenance_errors import (
    DuplicateAssetError,
    DuplicateTargetError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The candidate and run every generated record claims. Fixed, because the
#: properties are about selection over a bound tree and not about the
#: binding, which has its own cases.
CANDIDATE = "a" * 40
RUN_ID = "1234"

#: Package suffixes the ``linux`` platform stages, so a generated name is one
#: the claim check recognises as a package rather than an archive.
DEB_SUFFIX = ".deb"
RPM_SUFFIX = ".rpm"

_DIGEST = st.text(alphabet="0123456789abcdef", min_size=64, max_size=64)

#: A target key and the archive its configuration names. The archive carries
#: the key, so two generated targets never want the same archive name.
_TARGET = st.from_regex(r"linux-[a-z0-9]{1,8}", fullmatch=True)


@st.composite
def _target_set(draw: st.DrawFn) -> list[tuple[str, str]]:
    """Generate one to three distinct ``(target, archive)`` pairs.

    Returns
    -------
    list[tuple[str, str]]
        Each target key with the archive name that target stages, in the order
        drawn.
    """
    targets = draw(st.lists(_TARGET, min_size=1, max_size=3, unique=True))
    return [
        (target, f"netsuke-1.0.0-{index}-{target}.tar.gz")
        for index, target in enumerate(targets)
    ]


def _packages(index: int, target: str) -> tuple[str, str]:
    """Return the one ``.deb`` and one ``.rpm`` a target stages.

    Returns
    -------
    tuple[str, str]
        The two package names, both carrying the target so two generated
        targets never collide.
    """
    return (
        f"netsuke_1.0.0_{index}_{target}{DEB_SUFFIX}",
        f"netsuke-1.0.0-{index}.{target}{RPM_SUFFIX}",
    )


def _claims(
    pairs: cabc.Sequence[tuple[str, str]],
) -> list[tuple[ProducerProvenance, dict[str, str]]]:
    """Build one record per target over the names that target stages.

    Returns
    -------
    list[tuple[ProducerProvenance, dict[str, str]]]
        Each record with the name-to-digest mapping it claims.
    """
    claimed: list[tuple[ProducerProvenance, dict[str, str]]] = []
    for index, (target, archive) in enumerate(pairs):
        deb, rpm = _packages(index, target)
        assets = dict.fromkeys(
            (archive, f"{archive}.sha256", deb, rpm), f"{index:x}" * 64
        )
        record = ProducerProvenance(
            schema=1,
            commit=CANDIDATE,
            run_id=RUN_ID,
            run_attempt="1",
            target=target,
            platform="linux",
            assets=assets,
        )
        claimed.append((record, assets))
    return claimed


def _staged_for(assets: cabc.Iterable[str]) -> dict[str, list[str]]:
    """Stage one source per name, named after the name it carries.

    Returns
    -------
    dict[str, list[str]]
        Published name mapped to a single source token.
    """
    return {name: [f"staged:{name}"] for name in assets}


def _valid_tree(
    pairs: cabc.Sequence[tuple[str, str]],
) -> tuple[list[ProducerProvenance], list[str]]:
    """Build records for ``pairs`` and the names they claim, both sorted.

    Every case that damages a valid tree starts from this one. Damaging a tree
    the release would otherwise accept is what makes the refusal attributable
    to the damage rather than to some unrelated defect the generator happened
    to produce.

    Returns
    -------
    tuple[list[ProducerProvenance], list[str]]
        The records, and every name they claim in sorted order.
    """
    claimed = _claims(pairs)
    names = sorted(name for _record, assets in claimed for name in assets)
    return [record for record, _assets in claimed], names


def _select(
    records: cabc.Sequence[ProducerProvenance],
    staged: dict[str, list[str]],
    pairs: cabc.Sequence[tuple[str, str]],
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
        archives=dict(pairs),
        staged=staged,
    )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set())
def test_selection_declares_exactly_the_claimed_names(
    pairs: list[tuple[str, str]],
) -> None:
    """Declare every claimed name once, whatever the target set looks like."""
    claimed = _claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    selected = _select(
        [record for record, _assets in claimed], _staged_for(every_name), pairs
    )
    assert set(selected.declared) == every_name, (
        "selection must declare exactly the names its records claim"
    )
    assert len(selected.declared) == len(every_name), (
        "no declared name may appear twice"
    )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set())
def test_selection_carries_each_recorded_digest_unchanged(
    pairs: list[tuple[str, str]],
) -> None:
    """Carry each claimed name's digest through without substitution."""
    claimed = _claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    selected = _select(
        [record for record, _assets in claimed], _staged_for(every_name), pairs
    )
    for _record, assets in claimed:
        for name, digest in assets.items():
            assert selected.trusted[name] == digest, (
                f"{name} must keep the digest its producer recorded"
            )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set())
def test_selection_returns_each_names_own_source(
    pairs: list[tuple[str, str]],
) -> None:
    """Return the source the adapter observed for that name, not a sibling's."""
    claimed = _claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    staged = _staged_for(every_name)
    selected = _select([record for record, _assets in claimed], staged, pairs)
    for name in every_name:
        assert selected.sources[name] == staged[name][0], (
            f"{name} must keep the source staged under its own name"
        )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set())
def test_selection_is_indifferent_to_record_and_staging_order(
    pairs: list[tuple[str, str]],
) -> None:
    """Answer identically however the records and the staged mapping are ordered.

    The staged mapping is rebuilt in reverse and the records are reversed, so
    any answer that depended on insertion order would differ.
    """
    claimed = _claims(pairs)
    every_name = {name for _record, assets in claimed for name in assets}
    forward = _select(
        [record for record, _assets in claimed], _staged_for(every_name), pairs
    )
    backward = _select(
        [record for record, _assets in reversed(claimed)],
        dict(reversed(list(_staged_for(every_name).items()))),
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
@given(pairs=_target_set(), index=st.integers(min_value=0, max_value=2))
def test_a_missing_declared_name_is_refused(
    pairs: list[tuple[str, str]], index: int
) -> None:
    """Refuse a staged tree that omits one declared name.

    Generated from a valid tree with exactly one name removed, so the case is
    the omission and not some other defect.
    """
    records, names = _valid_tree(pairs)
    omitted = names[index % len(names)]
    staged = _staged_for(names)
    del staged[omitted]
    with pytest.raises(PayloadRefusedError, match="was not staged"):
        _select(records, staged, pairs)


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set(), index=st.integers(min_value=0, max_value=2))
def test_a_doubly_staged_declared_name_is_refused(
    pairs: list[tuple[str, str]], index: int
) -> None:
    """Refuse a staged tree that stages one declared name twice.

    Two sources claim one published name and nothing says which is the
    release's, so neither may be chosen.
    """
    records, names = _valid_tree(pairs)
    doubled = names[index % len(names)]
    staged = _staged_for(names)
    staged[doubled] = [*staged[doubled], f"duplicate:{doubled}"]
    with pytest.raises(PayloadRefusedError, match="was staged 2 times"):
        _select(records, staged, pairs)


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set())
def test_a_duplicated_target_is_refused(pairs: list[tuple[str, str]]) -> None:
    """Refuse two records that claim the same target."""
    claimed = _claims(pairs)
    records = [record for record, _assets in claimed]
    every_name = {name for _record, assets in claimed for name in assets}
    with pytest.raises(DuplicateTargetError):
        _select([*records, records[0]], _staged_for(every_name), pairs)


@settings(max_examples=50, deadline=None, derandomize=True)
@given(pairs=_target_set())
def test_a_package_name_two_targets_claim_is_refused(
    pairs: list[tuple[str, str]],
) -> None:
    """Refuse two configured targets claiming one package name.

    The second record trades its own ``.deb`` for the first target's. Every
    per-format count still reads one, and both archives stay with their own
    targets, so the per-target check passes and only the cross-record claim is
    wrong. That is what makes this the case for the union rather than for the
    binding.

    Generated target sets of one cannot exhibit the clash, so they are skipped
    rather than asserted vacuously.
    """
    claimed = _claims(pairs)
    if len(claimed) < 2:
        pytest.skip("a cross-target clash needs two configured targets")
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
            _staged_for(every_name),
            pairs,
        )
