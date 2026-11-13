"""Generate a valid multi-target release tree for the selection properties.

The properties in ``test_release_payload_selection_properties.py`` all start
from a tree the release would accept and then damage one thing about it.
Damaging a valid tree is what makes a refusal attributable to the damage rather
than to some unrelated defect the generator happened to produce, so the tree
itself has to be generated from the same rules the real records follow: one
archive per target, its sidecar, and one package per format its platform
stages.

Everything here is either a Hypothesis strategy or a derivation over names it
already drew. Nothing decides anything, and nothing asserts, so a property
still states its own contradiction and its own expectation.

Run via ``scripts/tests/test_release_payload_selection_properties.py``.
"""

import hashlib
import typing as typ

from hypothesis import strategies as st
from release_producer_provenance import ProducerProvenance

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

#: A target key and the archive its configuration names. The archive carries
#: the key, so two generated targets never want the same archive name.
_TARGET = st.from_regex(r"linux-[a-z0-9]{1,8}", fullmatch=True)


@st.composite
def target_set(
    draw: st.DrawFn, *, min_targets: int = 1, max_targets: int = 3
) -> list[tuple[str, str]]:
    """Generate distinct ``(target, archive)`` pairs.

    Parameters
    ----------
    min_targets
        The fewest targets to draw. A property about a *clash* between two
        targets asks for at least two, so it never has to discard an example
        it cannot use.
    max_targets
        The most targets to draw. A property that rewrites one record's claims
        asks for exactly two, because it builds its records pair by pair.

    Returns
    -------
    list[tuple[str, str]]
        Each target key with the archive name that target stages, in the order
        drawn.
    """
    targets = draw(
        st.lists(_TARGET, min_size=min_targets, max_size=max_targets, unique=True)
    )
    return [
        (target, f"netsuke-1.0.0-{index}-{target}.tar.gz")
        for index, target in enumerate(targets)
    ]


def packages(index: int, target: str) -> tuple[str, str]:
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


def claims(
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
        deb, rpm = packages(index, target)
        # Derived from the name so each claim carries its own digest. A shared
        # digest would let a case that swaps two names' digests pass: nothing
        # in the tree would then distinguish the swap from the original.
        assets = {
            name: hashlib.sha256(name.encode()).hexdigest()
            for name in (archive, f"{archive}.sha256", deb, rpm)
        }
        record = ProducerProvenance(
            commit=CANDIDATE,
            run_id=RUN_ID,
            run_attempt="1",
            target=target,
            platform="linux",
            assets=assets,
        )
        claimed.append((record, assets))
    return claimed


def staged_for(assets: cabc.Iterable[str]) -> dict[str, list[str]]:
    """Stage one source per name, named after the name it carries.

    Returns
    -------
    dict[str, list[str]]
        Published name mapped to a single source token.
    """
    return {name: [f"staged:{name}"] for name in assets}


def valid_tree(
    pairs: cabc.Sequence[tuple[str, str]],
) -> tuple[list[ProducerProvenance], list[str]]:
    """Build records for ``pairs`` and the names they claim, both sorted.

    Every case that damages a valid tree starts from this one.

    Returns
    -------
    tuple[list[ProducerProvenance], list[str]]
        The records, and every name they claim in sorted order.
    """
    claimed = claims(pairs)
    names = sorted(name for _record, assets in claimed for name in assets)
    return [record for record, _assets in claimed], names


def origins(
    claimed: cabc.Iterable[tuple[ProducerProvenance, dict[str, str]]],
) -> dict[str, str]:
    """Attribute each record's own packages to that record's target.

    Returns
    -------
    dict[str, str]
        Published package name mapped to the target that staged it, as a
        downloaded tree would report it.
    """
    return {
        name: record.target
        for record, assets in claimed
        for name in assets
        if name.endswith((DEB_SUFFIX, RPM_SUFFIX))
    }


__all__ = [
    "CANDIDATE",
    "DEB_SUFFIX",
    "RPM_SUFFIX",
    "RUN_ID",
    "claims",
    "origins",
    "packages",
    "staged_for",
    "target_set",
    "valid_tree",
]
