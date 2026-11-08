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

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_upload_declared_assets_cli.py``.
"""

import typing as typ

import release_reconcile_github
import release_upload_digests
import release_upload_remote

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_upload_plan import UploadRequest
    from release_upload_remote import SubprocessRunner

__all__ = [
    "observe_release",
    "plan_release",
    "reconcile",
    "should_upload",
]


def observe_release(
    tag: str, expected: cabc.Mapping[str, str], *, runner: SubprocessRunner
) -> dict[str, str | None]:
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
    dict[str, str | None]
        Attached asset name mapped to its digest, or ``None`` where neither
        the listing nor a download could establish one.

    Notes
    -----
    A listing that cannot be taken surfaces ``GhCommandFailedError`` or
    ``RemoteListingError`` from :mod:`release_upload_remote`. A download that
    fails does not: the name simply keeps its ``None``, which reconciliation
    reads as unverifiable rather than as a match on no evidence.

    Examples
    --------
    >>> observe_release(
    ...     "v1", {"a": "aa"},
    ...     runner=release_upload_remote.StaticRunner(
    ...         '{"assets": [{"name": "a", "digest": "sha256:aa"}]}'
    ...     ),
    ... )
    {'a': 'aa'}
    """
    listed = release_upload_remote.attached_digests(tag, runner=runner)
    wanted = release_reconcile_github.undigested(expected, listed)
    downloaded = (
        release_upload_digests.downloaded_digests(tag, wanted, runner=runner)
        if wanted
        else {}
    )
    return release_reconcile_github.present_assets(listed, downloaded)


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
        :func:`observe_release` returned them.

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
) -> tuple[tuple[str, release_reconcile_github.AssetAction], ...]:
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
    tuple[tuple[str, release_reconcile_github.AssetAction], ...]
        One ``(name, action)`` pair per declared asset, sorted by name.

    Notes
    -----
    The composition of :func:`observe_release` and :func:`plan_release`, kept
    so a caller that wants both does not restate the wiring.

    Examples
    --------
    An asset the release has not attached is planned for upload:

    >>> [action.value for _name, action in reconcile(
    ...     "v1", {"a": "aa"},
    ...     runner=release_upload_remote.StaticRunner('{"assets": []}'),
    ... )]
    ['upload']
    """
    return plan_release(
        expected, observed=observe_release(tag, expected, runner=runner)
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
