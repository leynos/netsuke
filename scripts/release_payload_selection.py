"""Choose the declared release payload from a staged tree before upload.

Publication receives a tree it did not build: one directory per producer,
hoisted archives at the root, and whatever else happened to be in the
workspace. Three independent accounts of the release meet here -- the staging
configuration, the producer records that travelled with the download, and the
names of the staged files -- and selection is the total decision over all
three.

The order of the checks is the point. Producer records are bound to the
candidate and the run *first*, so a tree left over from an earlier commit is
refused before any name it claims is trusted. Coverage is checked next, so a
target whose producer never reported is a refusal rather than a smaller
release. Only then is the declaration assembled, from configuration for the
archives and from the records for the packages the external actions name.

Staged files are then matched to that declaration by published name, which is
:mod:`release_payload_names`' job: non-release-shaped strays are excluded, and
release-shaped strays are refused.

Nothing here walks a tree, reads a file, or hashes bytes. The adapter observes
the tree and passes published name to source paths; this module decides. That
keeps every refusal reachable from a plain dictionary, which is what makes the
stale, duplicate, unrelated, and ambiguous cases testable at all.

The sources are opaque here and are returned untouched, so the adapter copies
exactly what selection approved.

Run via ``scripts/select_release_payload_cli.py``; behavioural coverage lives
in ``scripts/tests/test_release_payload_selection.py``.
"""

import dataclasses
import typing as typ

import release_declaration
import release_digest
import release_payload_names
import release_producer_provenance_checks
from release_payload_errors import (
    ClaimedDeclarationMismatchError,
    PayloadRefusedError,
    TargetPlatformMismatchError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_producer_provenance import ProducerProvenance

__all__ = [
    "ARCHIVE_SUFFIX",
    "PACKAGE_SUFFIXES",
    "PayloadSelection",
    "ReleaseIdentity",
    "is_release_shaped",
    "require_staged_digests",
    "select_payload",
]

#: Suffix of the cargo-binstall archive every target produces.
ARCHIVE_SUFFIX = release_payload_names.ARCHIVE_SUFFIX

#: Every package suffix the configured platforms stage.
PACKAGE_SUFFIXES = release_payload_names.PACKAGE_SUFFIXES

#: Re-exported so callers reach the predicate through the module that uses it.
is_release_shaped = release_payload_names.is_release_shaped


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseIdentity:
    """Name the release that a selected payload must serve.

    Attributes
    ----------
    candidate
        The full lowercase commit the release publishes.
    run_id
        The workflow run whose producers may contribute artefacts.
    target_config
        Parsed staging TOML naming the configured targets and platforms.
    """

    candidate: str
    run_id: str
    target_config: object


@dataclasses.dataclass(frozen=True, slots=True)
class PayloadSelection[Source]:
    """The declared assets selection approved, and where each one lives.

    Attributes
    ----------
    declared
        Every published asset name, sorted.
    trusted
        Published name mapped to the digest its producer recorded.
    sources
        Published name mapped to the staged file the adapter observed.
    """

    declared: tuple[str, ...]
    trusted: dict[str, str]
    sources: dict[str, Source]


def select_payload[Source](
    provenances: cabc.Iterable[ProducerProvenance],
    *,
    identity: ReleaseIdentity,
    archive_names: cabc.Iterable[str],
    staged: cabc.Mapping[str, cabc.Sequence[Source]],
) -> PayloadSelection[Source]:
    """Select the staged files that make up the declared release payload.

    Parameters
    ----------
    provenances
        Every validated producer record found in the downloaded tree.
    identity
        The candidate, run, and configured targets the release must serve.
    archive_names
        Archive names the configuration and manifest expect.
    staged
        Published name mapped to each observed file claiming it.

    Returns
    -------
    PayloadSelection
        The declared names, their producer-recorded digests, and one source
        path per name.

    Raises
    ------
    PayloadRefusedError
        If a declared name is unstaged or ambiguous, a staged name is unsafe,
        or a release-shaped file claims a name the declaration does not.
    ClaimedDeclarationMismatchError
        If the producer records do not claim exactly the declared names.

    Notes
    -----
    Binding, coverage, and naming refusals reach the caller from
    :mod:`release_producer_provenance_checks` and :mod:`release_declaration`
    with their own diagnoses: ``StaleCandidateError``, ``ForeignRunError``,
    ``MissingTargetError``, ``DuplicateTargetError``, ``DuplicateAssetError``,
    ``MissingPackageError``, ``DuplicatePackageError``, and
    ``TargetPlatformMismatchError`` from :func:`_require_platforms`. None is
    raised here, so Ruff's DOC501 keeps them out of the ``Raises`` section.

    Examples
    --------
    >>> from release_producer_provenance import ProducerProvenance
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ...     assets={
    ...         "netsuke.deb": "b" * 64, "netsuke.rpm": "c" * 64,
    ...         "netsuke.tar.gz": "d" * 64,
    ...         "netsuke.tar.gz.sha256": "e" * 64,
    ...     },
    ... )
    >>> selection = select_payload(
    ...     [provenance],
    ...     identity=ReleaseIdentity(
    ...         candidate="a" * 40, run_id="7",
    ...         target_config={"targets": {"linux-x86_64": {"platform": "linux"}}},
    ...     ),
    ...     archive_names=["netsuke.tar.gz"],
    ...     staged={
    ...         "netsuke.deb": ["deb"],
    ...         "netsuke.rpm": ["rpm"],
    ...         "netsuke.tar.gz": ["archive"],
    ...         "netsuke.tar.gz.sha256": ["sidecar"],
    ...     },
    ... )
    >>> selection.declared[0]
    'netsuke.deb'
    >>> selection.sources["netsuke.tar.gz"]
    'archive'
    """
    claims = list(provenances)
    _require_bound(claims, identity)
    targets = release_declaration.target_platforms(identity.target_config)
    release_producer_provenance_checks.require_target_coverage(
        claims, expected_targets=targets
    )
    _require_platforms(claims, targets)
    claimed = release_producer_provenance_checks.declared_names(claims)
    declared = release_declaration.declared_asset_names(
        archive_names=archive_names,
        target_config=identity.target_config,
        staged_package_names=claimed,
    )
    if set(declared) != set(claimed):
        raise ClaimedDeclarationMismatchError(declared, claimed)
    offenders = release_payload_names.describe_offenders(declared, staged)
    if offenders:
        raise PayloadRefusedError(offenders)
    return PayloadSelection(
        declared=declared,
        trusted=release_producer_provenance_checks.trusted_digests(claims),
        sources={name: staged[name][0] for name in declared},
    )


def require_staged_digests(
    computed: cabc.Mapping[str, str], *, trusted: cabc.Mapping[str, str]
) -> None:
    """Refuse staged bytes that differ from what the producers recorded.

    Parameters
    ----------
    computed
        Published name mapped to the digest of the bytes about to be uploaded.
    trusted
        Published name mapped to the digest its producer recorded.

    Raises
    ------
    PayloadRefusedError
        If any recorded asset was not hashed, or its bytes differ.

    Notes
    -----
    The comparison is against the record that travelled with the producers,
    not against anything derived from the payload. A tree that was replaced, or
    a package rebuilt from another commit, fails here -- before the first byte
    reaches the network.

    Examples
    --------
    >>> require_staged_digests({"a.deb": "b" * 64}, trusted={"a.deb": "b" * 64})
    >>> try:
    ...     require_staged_digests({"a.deb": "c" * 64}, trusted={"a.deb": "b" * 64})
    ... except PayloadRefusedError as refusal:
    ...     print("'a.deb'" in str(refusal))
    True
    """
    offenders = [
        f"{name!r} holds bytes its producer did not record"
        for name, recorded in sorted(trusted.items())
        if name not in computed
        or not release_digest.constant_time_equal(computed[name], recorded)
    ]
    if offenders:
        raise PayloadRefusedError(offenders)


def _require_bound(
    provenances: cabc.Iterable[ProducerProvenance], identity: ReleaseIdentity
) -> None:
    """Refuse a producer record from another commit or another run.

    Parameters
    ----------
    provenances
        Every validated producer record found in the downloaded tree.
    identity
        The candidate and run the release is publishing from.

    Notes
    -----
    A producer from another commit surfaces ``StaleCandidateError`` and one
    from another run ``ForeignRunError``, both from
    :func:`release_producer_provenance_checks.require_candidate`.

    Examples
    --------
    >>> _require_bound([], ReleaseIdentity("a" * 40, "7", {}))
    >>> from release_producer_provenance import ProducerProvenance
    >>> stale = ProducerProvenance(
    ...     schema=1, commit="b" * 40, run_id="7", run_attempt="1",
    ...     target="t", platform="linux", assets={"a.deb": "c" * 64},
    ... )
    >>> try:
    ...     _require_bound([stale], ReleaseIdentity("a" * 40, "7", {}))
    ... except ValueError as refusal:
    ...     print("not candidate" in str(refusal))
    True
    """
    for provenance in provenances:
        release_producer_provenance_checks.require_candidate(
            provenance, commit=identity.candidate, run_id=identity.run_id
        )


def _require_platforms(
    provenances: cabc.Iterable[ProducerProvenance],
    targets: cabc.Mapping[str, str],
) -> None:
    """Refuse a record whose platform its configured target does not serve.

    Parameters
    ----------
    provenances
        Every validated producer record found in the downloaded tree.
    targets
        Configured target key mapped to the platform it stages for.

    Raises
    ------
    TargetPlatformMismatchError
        If any record disagrees with the configuration.

    Examples
    --------
    >>> from release_producer_provenance import ProducerProvenance
    >>> wrong = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="macos", assets={"a.deb": "b" * 64},
    ... )
    >>> try:
    ...     _require_platforms([wrong], {"linux-x86_64": "linux"})
    ... except TargetPlatformMismatchError as refusal:
    ...     print("'macos'" in str(refusal))
    True
    """
    offenders = [
        (provenance.target, provenance.platform, targets[provenance.target])
        for provenance in provenances
        if targets[provenance.target] != provenance.platform
    ]
    if offenders:
        raise TargetPlatformMismatchError(offenders)
