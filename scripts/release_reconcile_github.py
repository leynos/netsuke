"""Decide which release assets to upload, skip, or refuse as conflicting.

A tag re-run must finish an interrupted release without re-uploading anything
already correct. The decision is made per asset by comparing two digests: the
one the release's own identity record says the bytes should have, and the one
GitHub reports for the asset already attached to the draft.

That comparison is the whole point. ``gh release upload --clobber`` replaces
whatever is there, so a re-run would overwrite a good asset with a newly built
one and call it a success -- and if the two were built from different commits,
the release would end up holding bytes from two commits while claiming one.
``clobber: false`` is not the answer either: it makes the second upload fail
without saying whether the asset that blocked it was the right one. Neither
flag is a reconciliation; comparing digests is.

The caller supplies already-normalised digest values. An asset that reports no
usable digest is a conflict rather than a match: nothing verifiable is not the
same as nothing wrong.

Nothing here performs network I/O; the caller lists the release and passes the
resulting mapping in.

Run via the release workflow's draft reconciliation; behavioural coverage
lives in ``scripts/tests/test_release_reconcile_github.py``.
"""

import enum
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


class AssetAction(enum.Enum):
    """What a release run should do about one asset name."""

    #: The asset is not attached yet, so it must be uploaded.
    UPLOAD = "upload"

    #: The attached asset's digest matches the expected one; leave it alone.
    SKIP_VERIFIED = "skip-verified"

    #: An attached asset's digest was read and differs from the expected one.
    CONFLICT = "conflict"

    #: An attached asset is present, but its digest could not be established.
    UNVERIFIABLE = "unverifiable"


def present_assets(
    listed: cabc.Mapping[str, str | None],
    downloaded: cabc.Mapping[str, str | None],
) -> dict[str, str | None]:
    """Combine a release listing with the digests a download established.

    Parameters
    ----------
    listed
        Attached asset name mapped to the digest the listing carried, or
        ``None`` where it carried none this release can compare.
    downloaded
        Attached asset name mapped to the digest of its downloaded bytes, or
        ``None`` where the download or the hash failed. May name assets the
        listing did not ask about; those are ignored.

    Returns
    -------
    dict[str, str | None]
        Every listed asset, with a ``None`` replaced by a downloaded digest
        where one exists.

    Notes
    -----
    Only a ``None`` in the listing is filled. A listing digest that is present
    is authoritative for its name: a download exists to supply evidence the
    listing withheld, not to overrule it. Taking the downloaded value whenever
    the two differ would let a truncated transfer contradict a digest GitHub
    itself reported.

    This is the join between the two effectful operations. Keeping it a pure
    function means the decision that consumes it can be tested from a plain
    mapping, with no listing, no transfer, and no stub in the way.

    Examples
    --------
    >>> present_assets({"a": None, "b": "bb"}, {"a": "aa", "b": "zz"})
    {'a': 'aa', 'b': 'bb'}
    >>> present_assets({"a": None}, {})
    {'a': None}
    """
    return {
        name: digest if digest is not None else downloaded.get(name)
        for name, digest in listed.items()
    }


def undigested(
    expected: cabc.Mapping[str, str], listed: cabc.Mapping[str, str | None]
) -> tuple[str, ...]:
    """Return the declared names a listing left without a digest.

    Parameters
    ----------
    expected
        Published name mapped to the digest the release must publish.
    listed
        Attached asset name mapped to the digest the listing carried.

    Returns
    -------
    tuple[str, ...]
        Sorted declared names that are attached but have no listing digest.

    Notes
    -----
    Only declared names are returned. An unrelated attached asset with no
    digest is not this step's concern, and downloading its bytes to learn a
    digest nothing consults would spend a transfer on a file the trusted
    record never named.

    Selecting *which* names need a transfer is a decision, so it lives here
    rather than in the boundary that performs it. The boundary then downloads
    exactly what this returns and asks nothing further.

    Examples
    --------
    >>> undigested(
    ...     {"a": "aa", "b": "bb"},
    ...     {"a": None, "b": "bb", "stray": None},
    ... )
    ('a',)
    """
    return tuple(
        sorted(name for name in expected if name in listed and listed[name] is None)
    )


def plan_asset_actions(
    *,
    expected: cabc.Mapping[str, str],
    present: cabc.Mapping[str, str | None],
) -> tuple[tuple[str, AssetAction], ...]:
    """Decide, per asset name, whether to upload, skip, or refuse.

    Parameters
    ----------
    expected
        Asset name to the bare hexadecimal digest it must have.
    present
        Asset name to its bare lowercase digest, already attached, or
        ``None`` where the adapter could not read a usable digest.

    Returns
    -------
    tuple[tuple[str, AssetAction], ...]
        One ``(name, action)`` pair per expected asset, sorted by name.

    Notes
    -----
    Only the *expected* names are decided. An asset attached to the draft that
    the identity record does not declare is not this function's business: the
    release declares its inventory, and an undeclared extra is a finding for
    the inventory check rather than something to reconcile here. Reporting it
    as UPLOAD would be wrong (it is present) and as CONFLICT would block a
    release over a file the plan never claimed.

    A name present without a digest is ``UNVERIFIABLE``, not ``CONFLICT``.
    Both stop the upload -- nothing may be overwritten on no evidence -- but
    they are different diagnoses with different remedies. A conflict says the
    release holds demonstrably wrong bytes and a person must decide what to do
    with them; an unverifiable name says the run could not tell, so the fault
    may be a listing or download problem rather than a bad release. Collapsing
    them would send an operator hunting for wrong bytes that may not exist.

    Sorting makes the result a value a caller can compare, log, or snapshot
    without depending on mapping order.

    Examples
    --------
    >>> expected = {"a.tar.gz": "aa", "a.tar.gz.sha256": "bb"}
    >>> plan_asset_actions(
    ...     expected=expected,
    ...     present={"a.tar.gz": "aa", "a.tar.gz.sha256": "zz"},
    ... )
    (('a.tar.gz', <AssetAction.SKIP_VERIFIED: 'skip-verified'>), \
('a.tar.gz.sha256', <AssetAction.CONFLICT: 'conflict'>))
    >>> plan_asset_actions(expected=expected, present={"a.tar.gz": None})
    (('a.tar.gz', <AssetAction.UNVERIFIABLE: 'unverifiable'>), \
('a.tar.gz.sha256', <AssetAction.UPLOAD: 'upload'>))
    >>> plan_asset_actions(expected=expected, present={})
    (('a.tar.gz', <AssetAction.UPLOAD: 'upload'>), \
('a.tar.gz.sha256', <AssetAction.UPLOAD: 'upload'>))
    """
    actions: list[tuple[str, AssetAction]] = []
    for name in sorted(expected):
        if name not in present:
            actions.append((name, AssetAction.UPLOAD))
            continue
        attached = present[name]
        wanted = expected[name].lower()
        if attached is None:
            actions.append((name, AssetAction.UNVERIFIABLE))
        elif attached == wanted:
            actions.append((name, AssetAction.SKIP_VERIFIED))
        else:
            actions.append((name, AssetAction.CONFLICT))
    return tuple(actions)


def uploads(
    actions: cabc.Iterable[tuple[str, AssetAction]],
) -> tuple[str, ...]:
    """Return the names a planned action set must upload, in order.

    Parameters
    ----------
    actions
        Output of :func:`plan_asset_actions`.

    Returns
    -------
    tuple[str, ...]
        Names planned for upload, in the order given.

    Notes
    -----
    The uploader sends exactly these names and nothing else, so an asset the
    release attached earlier is never re-sent. Re-uploading a verified asset
    would spend the transfer for no change, and re-uploading it *without*
    ``--clobber`` fails, which would turn an idempotent re-run into a red one.

    Examples
    --------
    >>> uploads([
    ...     ("a", AssetAction.CONFLICT),
    ...     ("b", AssetAction.UPLOAD),
    ...     ("c", AssetAction.SKIP_VERIFIED),
    ... ])
    ('b',)
    """
    return tuple(name for name, action in actions if action is AssetAction.UPLOAD)


def conflicts(actions: cabc.Iterable[tuple[str, AssetAction]]) -> tuple[str, ...]:
    """Return the names whose attached bytes were read and disagreed.

    Parameters
    ----------
    actions
        Output of :func:`plan_asset_actions`.

    Returns
    -------
    tuple[str, ...]
        Conflicting asset names, in the order given.

    Notes
    -----
    Separated from the planner so a caller can report every conflict in one
    message before stopping. A reconciler that raised on the first conflict
    would report one asset per re-run, which for a partial release is a slow
    way to learn what is wrong.

    An unverifiable name is *not* a conflict. It stops the upload just the
    same, but the operator is told a different thing: the bytes were never
    read, so there is no disagreement to describe.

    Examples
    --------
    >>> conflicts([
    ...     ("a", AssetAction.CONFLICT),
    ...     ("b", AssetAction.UPLOAD),
    ...     ("c", AssetAction.CONFLICT),
    ...     ("d", AssetAction.UNVERIFIABLE),
    ... ])
    ('a', 'c')
    """
    return tuple(name for name, action in actions if action is AssetAction.CONFLICT)


def unverifiable(
    actions: cabc.Iterable[tuple[str, AssetAction]],
) -> tuple[str, ...]:
    """Return the names whose attached digests could not be established.

    Parameters
    ----------
    actions
        Output of :func:`plan_asset_actions`.

    Returns
    -------
    tuple[str, ...]
        Unverifiable asset names, in the order given.

    Notes
    -----
    Kept apart from :func:`conflicts` because the two need different words in
    the diagnostic and different next steps from an operator. Grouping them
    would report a listing or download failure as though the release held
    demonstrably wrong bytes.

    Examples
    --------
    >>> unverifiable([
    ...     ("a", AssetAction.CONFLICT),
    ...     ("b", AssetAction.UNVERIFIABLE),
    ...     ("c", AssetAction.UPLOAD),
    ... ])
    ('b',)
    """
    return tuple(name for name, action in actions if action is AssetAction.UNVERIFIABLE)


def blocked(actions: cabc.Iterable[tuple[str, AssetAction]]) -> tuple[str, ...]:
    """Return every name that stops this run from uploading anything.

    Parameters
    ----------
    actions
        Output of :func:`plan_asset_actions`.

    Returns
    -------
    tuple[str, ...]
        Conflicting and unverifiable names together, in the order given.

    Notes
    -----
    A conflict blocks the run because uploading the other names would leave the
    release holding a mixture of this run's bytes and an earlier run's, which
    is harder to unwind than a refusal. An unverifiable name blocks it for the
    same reason: if the bytes already attached cannot be read, nothing shows
    the missing names are this run's either.

    ``actions`` is materialised once because it is annotated ``Iterable``. A
    caller that passes a generator would otherwise have it exhausted by
    :func:`conflicts`, and the unverifiable names would silently vanish from a
    result whose whole purpose is to say the run must stop.

    Examples
    --------
    >>> blocked([
    ...     ("a", AssetAction.CONFLICT),
    ...     ("b", AssetAction.UNVERIFIABLE),
    ...     ("c", AssetAction.UPLOAD),
    ... ])
    ('a', 'b')
    """
    settled = tuple(actions)
    return (*conflicts(settled), *unverifiable(settled))
