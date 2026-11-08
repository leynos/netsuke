"""Decide whether a downloaded tree is the release this run is publishing.

Selection asks three questions before it trusts a single name, and each has
its own answer. *Is this tree's work from the candidate?* A producer record
carries the commit and run it was built under, so a download left over from an
earlier commit, or carried over from another run, is refused before anything
it claims is read. *Did every configured target report?* A target whose
producer never arrived is a refusal, not a smaller release. *Does each record
describe the target it names?* A record's platform must be the one the staging
configuration gives its target.

The order is the point. A tree that is not this candidate's must be refused
before a missing target is diagnosed, because re-running the failed producer
would not fix it: the whole download is stale.

Nothing here reads a file or parses a record. Records arrive validated from
:mod:`release_producer_provenance`, and the refusals live in
:mod:`release_payload_errors` and :mod:`release_producer_provenance_errors`,
so this module decides and nothing else.

Run via ``scripts/release_payload_selection.py``; behavioural coverage lives in
``scripts/tests/test_release_payload_selection.py``.
"""

import dataclasses
import typing as typ

import release_declaration
import release_producer_provenance_checks
from release_payload_errors import TargetPlatformMismatchError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_producer_provenance import ProducerProvenance

__all__ = [
    "ReleaseIdentity",
    "require_bound_claims",
]


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


def require_bound_claims(
    claims: cabc.Sequence[ProducerProvenance], identity: ReleaseIdentity
) -> dict[str, str]:
    """Bind every record to the release, then to a covered, consistent target.

    Parameters
    ----------
    claims
        Every validated producer record found in the downloaded tree.
    identity
        The candidate, run, and configured targets the release must serve.

    Returns
    -------
    dict[str, str]
        Configured target key mapped to the platform it serves.

    Notes
    -----
    The order is the point. A tree left over from an earlier commit is refused
    before any name it claims is trusted, and a target whose producer never
    reported is a refusal rather than a smaller release.

    ``StaleCandidateError``, ``ForeignRunError``, ``MissingTargetError``,
    ``DuplicateTargetError``, and ``MalformedTargetError`` reach the caller
    from the policies this delegates to, so none is raised here.

    Examples
    --------
    >>> require_bound_claims([], ReleaseIdentity("a" * 40, "7", {"targets": {}}))
    Traceback (most recent call last):
        ...
    release_package_formats.MissingTargetsError: staging configuration defines \
no release targets
    """
    _require_bound(claims, identity)
    targets = release_declaration.target_platforms(identity.target_config)
    release_producer_provenance_checks.require_target_coverage(
        claims, expected_targets=targets
    )
    _require_platforms(claims, targets)
    return targets


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
