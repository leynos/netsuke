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

Every refusal below belongs to the helper that makes it, and the adapter
distinguishes them: ``StaleCandidateError``, ``ForeignRunError``,
``MissingTargetError``, ``DuplicateTargetError``, and
``TargetClaimMismatchError`` say the downloaded tree is not this release's,
while ``PayloadRefusedError`` and ``ClaimedDeclarationMismatchError`` say the
configuration and the records disagree. ``MissingPackageError``,
``DuplicatePackageError``, ``UnknownPlatformError``, and
``TargetPlatformMismatchError`` complete the inventory.

The sources are opaque here and are returned untouched, so the adapter copies
exactly what selection approved.

Run via ``scripts/select_release_payload_cli.py``; behavioural coverage lives
in ``scripts/tests/test_release_payload_selection.py``.
"""

import dataclasses
import typing as typ

import release_declaration
import release_digest
import release_payload_identity
import release_payload_names
import release_producer_claims
import release_producer_provenance_checks
from release_payload_errors import (
    ClaimedDeclarationMismatchError,
    PayloadRefusedError,
)

# Re-exported so the adapter and the suites keep one import site for the
# identity a selection is judged against, without this module's own policy
# code depending on the alias.
from release_payload_identity import ReleaseIdentity

# Re-exported so callers reach the naming policy through the module that
# consumes it, without this module's own policy code depending on the alias.
from release_payload_names import (
    ARCHIVE_SUFFIX,
    PACKAGE_SUFFIXES,
    is_release_shaped,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_producer_provenance import ProducerProvenance

__all__ = [
    "ARCHIVE_SUFFIX",
    "PACKAGE_SUFFIXES",
    "PayloadSelection",
    "ReleaseIdentity",
    "StagedTree",
    "is_release_shaped",
    "require_staged_digests",
    "select_payload",
]


@dataclasses.dataclass(frozen=True, slots=True)
class StagedTree[Source]:
    """Name everything the adapter observed in the downloaded tree.

    Attributes
    ----------
    archives
        Configured target key mapped to the archive name that target stages.
        A mapping rather than a flat name list so each record can be held to
        its *own* target's archive, which is what stops two same-platform
        records exchanging claims.
    staged
        Published name mapped to each observed file claiming it.
    package_origins
        Published package name mapped to the target key whose downloaded
        subtree staged it, as
        :func:`release_package_origins.observe_package_origins` reports it.
        ``None`` when no observation is available, which leaves the per-format
        count as the only package guarantee; an empty mapping is not the same
        thing, because it says the tree was walked and no package could be
        attributed.

    Notes
    -----
    One value rather than three parameters because the three answer one
    question -- what the tree holds -- so a caller that mixed an archive
    configuration with another walk's staged files would be describing two
    trees at once. Grouping them also keeps :func:`select_payload` inside the
    repository's argument ceiling as the observation grows.
    """

    archives: cabc.Mapping[str, str]
    staged: cabc.Mapping[str, cabc.Sequence[Source]]
    package_origins: cabc.Mapping[str, str] | None = None


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
    tree: StagedTree[Source],
) -> PayloadSelection[Source]:
    """Select the staged files that make up the declared release payload.

    Parameters
    ----------
    provenances
        Every validated producer record found in the downloaded tree.
    identity
        The candidate, run, and configured targets the release must serve.
    tree
        What the adapter observed: each target's archive name, the files
        staged under each published name, and which subtree staged each
        package.

    Returns
    -------
    PayloadSelection
        The declared names, their producer-recorded digests, and one source
        path per name.

    Notes
    -----
    Every refusal reaches the caller from one of the helpers below, so none is
    listed under ``Raises``. The module docstring inventories them.

    Examples
    --------
    >>> from release_producer_provenance import ProducerProvenance
    >>> provenance = ProducerProvenance(
    ...     commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ...     assets={"pkg.deb": "b" * 64, "pkg.rpm": "c" * 64,
    ...             "pkg.tar.gz": "d" * 64, "pkg.tar.gz.sha256": "e" * 64},
    ... )
    >>> selection = select_payload(
    ...     [provenance],
    ...     identity=ReleaseIdentity(
    ...         "a" * 40, "7", {"targets": {"linux-x86_64": {"platform": "linux"}}}
    ...     ),
    ...     tree=StagedTree(
    ...         archives={"linux-x86_64": "pkg.tar.gz"},
    ...         staged={"pkg.deb": ["deb"], "pkg.rpm": ["rpm"],
    ...                 "pkg.tar.gz": ["archive"],
    ...                 "pkg.tar.gz.sha256": ["sidecar"]},
    ...     ),
    ... )
    >>> selection.sources["pkg.tar.gz"]
    'archive'
    """
    claims = list(provenances)
    targets = release_payload_identity.require_bound_claims(claims, identity)
    claimed = _require_claims(claims, targets=targets, tree=tree)
    declared = _require_declaration(
        archive_names=tree.archives.values(),
        targets=targets,
        claimed=claimed,
    )
    _require_staged(declared, tree.staged)
    return PayloadSelection(
        declared=declared,
        trusted=release_producer_provenance_checks.trusted_digests(claims),
        sources={name: tree.staged[name][0] for name in declared},
    )


def _require_claims[Source](
    claims: cabc.Sequence[ProducerProvenance],
    *,
    targets: cabc.Mapping[str, str],
    tree: StagedTree[Source],
) -> tuple[str, ...]:
    """Hold each record to its own target's archive and package formats.

    Parameters
    ----------
    claims
        Every validated producer record found in the downloaded tree.
    targets
        Target key mapped to the platform the configuration names for it.
    tree
        What the adapter observed, already bound to these claims. Its
        ``package_origins`` decides whether a package may be refused for
        coming from a *sibling's* subtree or from none at all; without it the
        count checks stand and nothing weaker, because a count is all an
        unobserved tree can support.

    Returns
    -------
    tuple[str, ...]
        Every published name the producers claim, flat and sorted.

    Notes
    -----
    ``TargetClaimMismatchError`` surfaces from
    :func:`release_producer_claims.require_target_claims` and
    ``DuplicateAssetError`` from
    :func:`release_producer_provenance_checks.declared_names`; neither is
    raised here.

    Examples
    --------
    >>> _require_claims(
    ...     [],
    ...     targets={},
    ...     tree=StagedTree(archives={}, staged={}),
    ... )
    ()
    """
    release_producer_claims.require_target_claims(
        claims,
        archives=tree.archives,
        targets=targets,
        package_origins=tree.package_origins,
    )
    return release_producer_provenance_checks.declared_names(claims)


def _require_declaration(
    *,
    archive_names: cabc.Iterable[str],
    targets: cabc.Mapping[str, str],
    claimed: tuple[str, ...],
) -> tuple[str, ...]:
    """Assemble the declaration and require it to be exactly what was claimed.

    Returns
    -------
    tuple[str, ...]
        Sorted archive names, sidecars, and observed packages.

    Raises
    ------
    ClaimedDeclarationMismatchError
        If the records do not claim exactly the declared names.

    Notes
    -----
    The per-target check has already bound each record, so a remaining
    disagreement is between the *configuration* and the records as a whole.
    ``MissingPackageError``, ``DuplicatePackageError``, and
    ``UnknownPlatformError`` surface from :mod:`release_declaration` instead.

    The declaration counts targets by platform, so it takes the typed record.
    The per-target claim check needs the configured keys instead, which the
    typed record does not carry, so the caller passes the same mapping it
    already derived from one parsed configuration.

    Examples
    --------
    >>> _require_declaration(
    ...     archive_names=["crate.tar.gz"],
    ...     targets={"linux": "linux"},
    ...     claimed=("crate-1.deb", "crate-1.rpm", "crate.tar.gz",
    ...              "crate.tar.gz.sha256"),
    ... )[:2]
    ('crate-1.deb', 'crate-1.rpm')
    """
    declared = release_declaration.declared_asset_names(
        archive_names=archive_names,
        targets=release_declaration.StagingTargets.from_targets(
            release_declaration.StagingTarget(platform) for platform in targets.values()
        ),
        staged_package_names=claimed,
    )
    if set(declared) != set(claimed):
        raise ClaimedDeclarationMismatchError(declared, claimed)
    return declared


def _require_staged[Source](
    declared: cabc.Iterable[str], staged: cabc.Mapping[str, cabc.Sequence[Source]]
) -> None:
    """Refuse a staged tree that does not hold exactly the declared files.

    Raises
    ------
    PayloadRefusedError
        If any declared name is unstaged or ambiguous, a staged name is unsafe,
        or a release-shaped file claims a name the declaration does not.

    Examples
    --------
    >>> _require_staged(("a.deb",), {"a.deb": ["x"]})
    >>> try:
    ...     _require_staged(("a.deb",), {})
    ... except PayloadRefusedError as refusal:
    ...     print("was not staged" in str(refusal))
    True
    """
    offenders = release_payload_names.describe_offenders(declared, staged)
    if offenders:
        raise PayloadRefusedError(offenders)


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
