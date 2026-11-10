"""Observe a release, then decide what the observation means for the payload.

Two kinds of work meet in an upload run, and this module keeps them apart. The
first is effectful: list the release, and -- only for the declared names whose
listing carried no digest -- download the asset and hash it. The second is a
read-only query over the result. Splitting them means the decision can be
stated as a table of observations and tested without a stub, while exactly one
function reaches the outside world.

The listing and the download both go through :mod:`release_upload_remote`, so
a test replaces one seam and no test reaches GitHub. Which names are
downloaded is not decided in the boundary: it is
:func:`release_reconcile_github.undigested`' answer, and the download is
skipped entirely when that answer is empty.

:func:`reconcile` composes the two for the caller that wants both, so the CLI
states its orchestration once rather than restating the wiring at each site.

An observation carries more than digests. Where a digest could not be
established, :class:`release_upload_failures.DigestFailure` records which kind
of failure it was, in a vocabulary small enough to publish: a diagnostic that
quoted the failing command's output could carry a URL, a temporary path, or
whatever else the tool chose to print, and a retained plan outlives the run.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_upload_declared_assets_cli.py``.
"""

import dataclasses
import typing as typ

import release_reconcile_github
import release_upload_digests
import release_upload_remote

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_upload_failures import DigestFailure
    from release_upload_plan import UploadRequest
    from release_upload_remote import SubprocessRunner

__all__ = [
    "Observation",
    "Settlement",
    "observe_release",
    "plan_release",
    "reconcile",
    "settle_observation",
    "should_upload",
]


@dataclasses.dataclass(frozen=True, slots=True)
class Observation:
    """Hold what one release listing established and what it could not.

    Attributes
    ----------
    digests
        Attached asset name mapped to its digest, or ``None`` where neither
        the listing nor a download could establish one.
    failures
        Asset name mapped to the bounded category explaining why its digest is
        ``None``. Empty when every name the download was asked about produced
        one, and never naming a name that has a digest.
    """

    digests: dict[str, str | None]
    failures: dict[str, DigestFailure]


@dataclasses.dataclass(frozen=True, slots=True)
class Settlement:
    """Hold what an upload run decided and what it learned about the release.

    Attributes
    ----------
    actions
        The final per-asset plan, sorted by name. After a transfer this
        describes what the release holds rather than what was sent.
    failures
        Asset name mapped to the bounded category explaining a digest that
        could not be established. Reported beside the plan because the plan
        alone says *that* an asset is unverifiable, never why.

    Notes
    -----
    One value rather than two return values, so a caller cannot keep the plan
    and drop the reasons -- which is exactly the diagnostic an operator needs
    to choose between retrying a transfer and investigating a release.
    """

    actions: tuple[tuple[str, release_reconcile_github.AssetAction], ...]
    failures: dict[str, DigestFailure]


def observe_release(
    tag: str, expected: cabc.Mapping[str, str], *, runner: SubprocessRunner
) -> Observation:
    """Observe the release: list its assets, then download the digests it hid.

    Parameters
    ----------
    tag
        The validated release tag to list.
    expected
        Published name mapped to the digest the trusted record demands.
    runner
        The injected subprocess seam.

    Returns
    -------
    Observation
        Each attached asset's digest where one was established, and the
        category explaining each one that was not.

    Notes
    -----
    A listing that cannot be taken surfaces ``GhCommandFailedError`` or
    ``RemoteListingError`` from :mod:`release_upload_remote`. A download that
    fails does not: the name simply keeps its ``None``, which reconciliation
    reads as unverifiable rather than as a match on no evidence, while the
    category says whether a retry is the next thing to try.

    Examples
    --------
    >>> observe_release(
    ...     "v1", {"a": "aa"},
    ...     runner=release_upload_remote.StaticRunner(
    ...         '{"assets": [{"name": "a", "digest": "sha256:aa"}]}'
    ...     ),
    ... )
    Observation(digests={'a': 'aa'}, failures={})
    """
    listed = release_upload_remote.attached_digests(tag, runner=runner)
    wanted = release_reconcile_github.undigested(expected, listed)
    evidence = (
        release_upload_digests.downloaded_digests(tag, wanted, runner=runner)
        if wanted
        else release_upload_digests.DownloadEvidence(digests={}, failures={})
    )
    return Observation(
        digests=release_reconcile_github.present_assets(listed, evidence.digests),
        failures=evidence.failures,
    )


def plan_release(
    expected: cabc.Mapping[str, str], *, observed: cabc.Mapping[str, str | None]
) -> tuple[tuple[str, release_reconcile_github.AssetAction], ...]:
    """Decide every declared asset's fate from an already-taken observation.

    Parameters
    ----------
    expected
        Published name mapped to the digest the trusted record demands.
    observed
        The release's attached assets and their digests, as
        :func:`observe_release` returned them in its ``digests``.

    Returns
    -------
    tuple[tuple[str, release_reconcile_github.AssetAction], ...]
        One ``(name, action)`` pair per declared asset, sorted by name.

    Notes
    -----
    Reaches no network, reads no file, and holds no state.

    Examples
    --------
    >>> [action.value for _name, action in plan_release(
    ...     {"a": "aa", "b": "bb"}, observed={"a": "aa", "b": None}
    ... )]
    ['skip-verified', 'unverifiable']
    """
    return release_reconcile_github.plan_asset_actions(
        expected=expected, present=observed
    )


def reconcile(
    tag: str, expected: cabc.Mapping[str, str], *, runner: SubprocessRunner
) -> Settlement:
    """Observe the release and plan its declared assets in one pass.

    Parameters
    ----------
    tag
        The validated release tag to observe.
    expected
        Published name mapped to the digest the trusted record demands.
    runner
        The injected subprocess seam.

    Returns
    -------
    Settlement
        The per-asset plan and the bounded reasons behind any asset whose
        digest could not be established.

    Notes
    -----
    :func:`observe_release` is the only function here that reaches outside the
    process; this one composes it with the pure :func:`plan_release` and exists
    so a caller that wants both states the wiring once. The failure
    categories travel with the plan because they live only in the observation;
    a caller given the plan alone could say an asset was unverifiable but not
    whether the remedy is a retry. A caller that needs to interleave a transfer
    between the effect and the decision calls this function once before the
    transfer and once after, as :func:`upload_declared_assets_cli.settle` does
    across its re-read.

    Examples
    --------
    An asset the release has not attached is planned for upload:

    >>> settlement = reconcile(
    ...     "v1", {"a": "aa"},
    ...     runner=release_upload_remote.StaticRunner('{"assets": []}'),
    ... )
    >>> [action.value for _name, action in settlement.actions]
    ['upload']
    >>> settlement.failures
    {}
    """
    return settle_observation(
        observe_release(tag, expected, runner=runner), expected=expected
    )


def settle_observation(
    observation: Observation, *, expected: cabc.Mapping[str, str]
) -> Settlement:
    """Turn one taken observation into the plan and the reasons behind it.

    Parameters
    ----------
    observation
        What one listing and its downloads established, as
        :func:`observe_release` returned it.
    expected
        Published name mapped to the digest the trusted record demands. Passed
        rather than read back out of the observation, because an observation
        is a statement about the release: what the *payload* should hold is the
        caller's, and a plan derived from a listing nobody constrained would
        decide about names the release merely happens to carry.

    Returns
    -------
    Settlement
        The per-asset plan, sorted by name, and the bounded failure
        categories carried beside it.

    Notes
    -----
    The boundary between the effectful half of an upload run and its decision.
    Taking the observation rather than the tag is what lets a caller decide
    from an observation it already has -- after a transfer, say -- without
    listing the release a second time, and what lets the decision be tested
    from a handwritten :class:`Observation` with no stub at all.

    Examples
    --------
    >>> settlement = settle_observation(
    ...     Observation(digests={"a": "aa", "b": None}, failures={}),
    ...     expected={"a": "aa", "b": "bb"},
    ... )
    >>> [action.value for _name, action in settlement.actions]
    ['skip-verified', 'unverifiable']
    """
    return Settlement(
        actions=plan_release(expected, observed=observation.digests),
        failures=observation.failures,
    )


def should_upload(
    request: UploadRequest,
    actions: cabc.Iterable[tuple[str, release_reconcile_github.AssetAction]],
    pending: cabc.Iterable[str],
) -> bool:
    """Report whether this run has a transfer to make.

    Parameters
    ----------
    request
        The run's request, whose dry-run flag is what this reads.
    actions
        The plan the observation produced.
    pending
        The names the plan asks to upload.

    Returns
    -------
    bool
        ``False`` for a dry run, a plan with nothing to upload, or a plan that
        already contains a blocker.

    Notes
    -----
    A conflict blocks every upload even when other names are missing. Sending
    the missing ones first would leave the release holding a mixture of this
    run's bytes and an earlier run's, which is harder to unwind than a refusal.
    An unverifiable asset blocks it for the same reason, and
    :func:`release_reconcile_github.blocked` is what pairs the two so this
    predicate cannot come to know one and forget the other.

    Examples
    --------
    >>> from pathlib import Path
    >>> from release_reconcile_github import AssetAction
    >>> from release_upload_plan import UploadRequest
    >>> def request(*, is_dry_run: bool) -> UploadRequest:
    ...     return UploadRequest("v1", Path(), Path(), Path(), Path(), is_dry_run)
    >>> should_upload(request(is_dry_run=False),
    ...               [("a", AssetAction.UPLOAD)], ("a",))
    True
    >>> should_upload(request(is_dry_run=True),
    ...               [("a", AssetAction.UPLOAD)], ("a",))
    False
    >>> should_upload(request(is_dry_run=False),
    ...               [("a", AssetAction.UNVERIFIABLE),
    ...                ("b", AssetAction.UPLOAD)], ("b",))
    False
    """
    return (
        not request.is_dry_run
        and bool(tuple(pending))
        and not tuple(release_reconcile_github.blocked(actions))
    )
