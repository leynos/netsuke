"""Render a download plan as the tab-separated text the workflow step reads.

The plan is recorded as ``id<TAB>name<NEWLINE>`` per asset and read back field
by field, so an asset name containing a tab, a line feed, or a carriage return
cannot be represented in it. A line feed is the dangerous one: it would forge an
extra record, and the download step would fetch an asset the listing never
planned. The refusal lives here, beside the rendering it protects, rather than
in :mod:`release_draft_assets`, because it is a property of this file format
and not of a release: a future consumer that addresses assets by number alone
would not inherit it.

Escaping is not an option. The step splits on tabs and newlines and never
unquotes, so an escaped name would be downloaded under a name the trusted
record does not carry; refusing keeps the plan and the filesystem in agreement.

Nothing here touches a file: :mod:`release_draft_assets_cli` writes the text.
Behavioural coverage lives in ``scripts/tests/test_release_download_plan.py``.
"""

import typing as typ

import release_draft_assets

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Characters that would split or truncate one line of the download plan, which
#: is written as ``id<TAB>name<NEWLINE>`` and read back field by field.
PLAN_DELIMITERS = ("\t", "\n", "\r")


class PlanFormatError(release_draft_assets.DraftAssetError):
    """Report an asset name the tab-separated plan cannot carry."""


def _unrepresentable_names(
    assets: cabc.Sequence[release_draft_assets.Asset],
) -> list[str]:
    """Return the names this format would split or truncate.

    Returns
    -------
    list[str]
        Every name containing a tab, a line feed, or a carriage return, in the
        order given; empty when every name is renderable.
    """
    return [asset.name for asset in assets if set(asset.name) & set(PLAN_DELIMITERS)]


def format_plan(assets: cabc.Sequence[release_draft_assets.Asset]) -> str:
    r"""Render planned assets as one newline-terminated record each.

    Parameters
    ----------
    assets
        The assets to download, already decided by
        :func:`release_draft_assets.plan_downloads`.

    Returns
    -------
    str
        One ``id<TAB>name`` line per asset, in the order given.

    Raises
    ------
    PlanFormatError
        If a name cannot be carried by the format, so a file written from this
        text can never disagree with the listing it came from.

    Examples
    --------
    >>> format_plan([release_draft_assets.Asset(11, "a.tar.gz",
    ...                                          is_stored=True, size=5)])
    '11\ta.tar.gz\n'
    """
    unrepresentable = _unrepresentable_names(assets)
    if unrepresentable:
        msg = (
            "release asset names cannot be represented in the download plan: "
            + ", ".join(repr(name) for name in unrepresentable)
        )
        raise PlanFormatError(msg)
    return "".join(f"{asset.identifier}\t{asset.name}\n" for asset in assets)
