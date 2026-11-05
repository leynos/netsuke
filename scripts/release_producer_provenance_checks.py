"""Judge producer records against the release they claim to serve.

One validated record says what a single producer built. Publication has three
questions that only make sense across records or against the release's own
identity: is this the candidate we are publishing, was it written by this run,
and does the set of records cover every configured target exactly once while
claiming each published name exactly once?

Every check here is a total function over already-validated claims. Nothing
reads a file, parses JSON, or hashes bytes; the adapters own those boundaries.
The refusal vocabulary lives in :mod:`release_producer_provenance_errors` so
this module and its readers share one set of diagnoses without importing the
record reader.

A record binds bytes to a producer's claim, not to a proven origin. These
checks decide whether a claim is *relevant* to this release; they do not make
it true.

Run via the release publication adapters; behavioural coverage lives in
``scripts/tests/test_release_producer_provenance.py``.
"""

import typing as typ

from release_producer_provenance import RUN_ID_MEMBER, ProducerProvenance
from release_producer_provenance_errors import (
    DuplicateAssetError,
    DuplicateTargetError,
    ForeignRunError,
    MissingTargetError,
    StaleCandidateError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "declared_names",
    "require_candidate",
    "require_target_coverage",
    "trusted_digests",
]


def require_candidate(
    provenance: ProducerProvenance, *, commit: str, run_id: str
) -> None:
    """Refuse a record built from another commit or written by another run.

    Parameters
    ----------
    provenance
        The producer claim to check.
    commit
        The candidate commit the release declared.
    run_id
        The workflow run the release is publishing from.

    Raises
    ------
    StaleCandidateError
        If the producer built a different commit.
    ForeignRunError
        If the producer recorded a different run identity.

    Notes
    -----
    The run *attempt* is deliberately not compared. Retrying a job re-runs it
    from the same commit, and a retried producer legitimately replaces the
    artefacts its first attempt uploaded; refusing a later attempt would make
    a failed job unrecoverable.

    Examples
    --------
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux", assets={"a.deb": "b" * 64},
    ... )
    >>> require_candidate(provenance, commit="a" * 40, run_id="7")
    >>> try:
    ...     require_candidate(provenance, commit="c" * 40, run_id="7")
    ... except StaleCandidateError as error:
    ...     print(error)  # doctest: +ELLIPSIS
    producer built commit 'aaaa..., not candidate 'cccc...
    """
    if provenance.commit != commit:
        raise StaleCandidateError(provenance.commit, commit)
    if provenance.run_id != run_id:
        raise ForeignRunError(provenance.run_id, expected=run_id, member=RUN_ID_MEMBER)


def require_target_coverage(
    provenances: cabc.Iterable[ProducerProvenance],
    *,
    expected_targets: cabc.Iterable[str],
) -> None:
    """Refuse a producer set that does not cover every configured target once.

    Parameters
    ----------
    provenances
        Every producer claim that arrived.
    expected_targets
        The target keys the staging configuration declares.

    Raises
    ------
    DuplicateTargetError
        If more than one record claims one configured target.
    MissingTargetError
        If a configured target has no producer record, or a record names a
        target the configuration does not declare.

    Examples
    --------
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux", assets={"a.deb": "b" * 64},
    ... )
    >>> require_target_coverage([provenance], expected_targets=["linux-x86_64"])
    >>> try:
    ...     require_target_coverage(
    ...         [provenance], expected_targets=["linux-x86_64", "macos-x86_64"]
    ...     )
    ... except MissingTargetError as error:
    ...     print(error)
    no producer record covers targets ['macos-x86_64']
    """
    expected = sorted(set(expected_targets))
    claimed = [provenance.target for provenance in provenances]
    duplicated = sorted({target for target in claimed if claimed.count(target) > 1})
    if duplicated:
        raise DuplicateTargetError(duplicated)
    # A record for an undeclared target is not a coverage gap in the
    # configuration; it is an artefact of a different release, and it is
    # reported with the same "these targets are not covered" diagnosis so the
    # operator sees one, complete, symmetric set of offenders.
    uncovered = sorted(set(expected) - set(claimed))
    undeclared = sorted(set(claimed) - set(expected))
    if uncovered or undeclared:
        raise MissingTargetError([*uncovered, *undeclared])


def declared_names(
    provenances: cabc.Iterable[ProducerProvenance],
) -> tuple[str, ...]:
    """Return every published name the producers claim, sorted and unique.

    Returns
    -------
    tuple[str, ...]
        The flat consumer set, in sorted order.

    Raises
    ------
    DuplicateAssetError
        If two producers claim one published name.

    Examples
    --------
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ...     assets={"b.deb": "b" * 64, "a.rpm": "c" * 64},
    ... )
    >>> declared_names([provenance])
    ('a.rpm', 'b.deb')
    """
    names = [name for provenance in provenances for name in provenance.assets]
    duplicated = sorted({name for name in names if names.count(name) > 1})
    if duplicated:
        raise DuplicateAssetError(duplicated)
    return tuple(sorted(names))


def trusted_digests(
    provenances: cabc.Iterable[ProducerProvenance],
) -> dict[str, str]:
    """Return the union of every producer's claimed digests.

    Returns
    -------
    dict[str, str]
        Published asset name mapped to the digest its producer recorded.

    Notes
    -----
    A name two producers claim surfaces as ``DuplicateAssetError`` from
    :func:`declared_names`, which this function delegates to. Two claims for
    one name would make the trusted record describe bytes neither producer is
    answerable for alone, so the union is never attempted.

    Examples
    --------
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux", assets={"a.deb": "b" * 64},
    ... )
    >>> trusted_digests([provenance])
    {'a.deb': 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
    """
    declared_names(provenances)
    digests: dict[str, str] = {}
    for provenance in provenances:
        digests.update(provenance.assets)
    return dict(sorted(digests.items()))
