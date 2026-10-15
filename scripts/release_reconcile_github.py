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

GitHub reports an asset's digest as ``sha256:<hex>``, and every asset carries
one -- confirmed across all 29 assets of ``v0.1.0-beta4`` on 2026-10-15. So the
prefix is stripped before comparison, and an asset that reports no digest is a
conflict rather than a match: nothing verifiable is not the same as nothing
wrong.

Nothing here performs network I/O; the caller lists the release and passes the
resulting mapping in.

Run via the release workflow's draft reconciliation; behavioural coverage
lives in ``scripts/tests/test_release_reconcile_github.py``.
"""

import enum
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Prefix GitHub puts in front of an asset's hex SHA-256.
DIGEST_PREFIX = "sha256:"


class AssetAction(enum.Enum):
    """What a release run should do about one asset name."""

    #: The asset is not attached yet, so it must be uploaded.
    UPLOAD = "upload"

    #: The attached asset's digest matches the expected one; leave it alone.
    SKIP_VERIFIED = "skip-verified"

    #: An asset is attached whose digest differs, or cannot be read.
    CONFLICT = "conflict"


class AssetConflictError(ValueError):
    """Report assets that a release run must not overwrite."""


def normalise_digest(reported: str | None) -> str | None:
    """Return a GitHub-reported digest as bare lowercase hexadecimal.

    Parameters
    ----------
    reported
        The digest GitHub reports for an asset, or ``None``.

    Returns
    -------
    str | None
        The hexadecimal digest, or ``None`` when there is nothing to read.

    Notes
    -----
    GitHub answers ``sha256:<hex>`` while the identity record stores the bare
    hexadecimal that ``sha256sum`` printed. Comparing the two without
    normalising reports every matching asset as a conflict -- so this strips
    the prefix and lowercases, and returns ``None`` for anything that does not
    carry the prefix, rather than passing the raw string through to be
    compared and mismatched.

    Examples
    --------
    >>> normalise_digest("sha256:" + "AB" * 32)
    'abababababababababababababababababababababababababababababababab'
    >>> normalise_digest(None) is None
    True
    >>> normalise_digest("0b4da8d445") is None
    True
    """
    if reported is None or not reported.startswith(DIGEST_PREFIX):
        return None
    digest = reported[len(DIGEST_PREFIX) :].strip().lower()
    return digest or None


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
        Asset name to the digest GitHub reports, already attached, or
        ``None`` where the reported value is unusable.

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

    Sorting makes the result a value a caller can compare, log, or snapshot
    without depending on mapping order.

    Examples
    --------
    >>> expected = {"a.tar.gz": "aa", "a.tar.gz.sha256": "bb"}
    >>> plan_asset_actions(
    ...     expected=expected,
    ...     present={"a.tar.gz": "sha256:aa", "a.tar.gz.sha256": "sha256:zz"},
    ... )
    (('a.tar.gz', <AssetAction.SKIP_VERIFIED: 'skip-verified'>), \
('a.tar.gz.sha256', <AssetAction.CONFLICT: 'conflict'>))
    >>> plan_asset_actions(expected=expected, present={})
    (('a.tar.gz', <AssetAction.UPLOAD: 'upload'>), \
('a.tar.gz.sha256', <AssetAction.UPLOAD: 'upload'>))
    """
    actions: list[tuple[str, AssetAction]] = []
    for name in sorted(expected):
        if name not in present:
            actions.append((name, AssetAction.UPLOAD))
            continue
        attached = normalise_digest(present[name])
        wanted = expected[name].lower()
        if attached is not None and attached == wanted:
            actions.append((name, AssetAction.SKIP_VERIFIED))
        else:
            actions.append((name, AssetAction.CONFLICT))
    return tuple(actions)


def conflicts(actions: typ.Iterable[tuple[str, AssetAction]]) -> tuple[str, ...]:
    """Return the names a planned action set refuses to overwrite.

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

    Examples
    --------
    >>> conflicts([
    ...     ("a", AssetAction.CONFLICT),
    ...     ("b", AssetAction.UPLOAD),
    ...     ("c", AssetAction.CONFLICT),
    ... ])
    ('a', 'c')
    """
    return tuple(name for name, action in actions if action is AssetAction.CONFLICT)
