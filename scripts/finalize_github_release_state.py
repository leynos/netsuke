"""Describe the release facts a finalization run decides from.

Two decisions stand between a verified release and a public one: whether the
release is already public, and what it looked like after the API call that
un-drafts it. A third question is asked of the same record before that call:
whether the release still attaches the assets the reconciliation plan verified.
The plan answers for a snapshot taken earlier, so a name it accepted and the
release no longer carries means the release moved under the run. All three are
decided from decoded values rather than from the live API, so the partial and
interrupted states that are hardest to arrange against GitHub can be exercised
directly.

Nothing here reads a file or runs a command. The adapter decodes the API
response, then asks these functions what it means; that split lets this suite
state a policy while the adapter's suite proves the wiring. The plan's own
verdict lives in ``finalize_github_release_plan``.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_state.py``.
"""

import collections.abc as cabc
import dataclasses
import enum

from finalize_github_release_plan import FinalizePolicyError


class Readback(enum.Enum):
    """What a release looks like after the un-drafting call."""

    #: The release is public and its prerelease state matches the candidate.
    FINALIZED = "finalized"

    #: The release is still a draft, so the call did not take effect.
    STILL_DRAFT = "draft"

    #: The release is public but its prerelease state contradicts the candidate.
    PRERELEASE_MISMATCH = "prerelease-mismatch"


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseState:
    """Record the release facts one ``gh release view`` response reports.

    Attributes
    ----------
    tag
        The tag the response named, already checked against the requested one.
    is_draft
        Whether GitHub still holds the release as a draft.
    is_prerelease
        Whether GitHub marks the release as a prerelease.
    assets
        Attached asset name to the digest GitHub reported, or ``None`` where
        the response carried no usable digest.

    Examples
    --------
    >>> state = ReleaseState("v1.2.3", True, False, {"a": "sha256:ab"})
    >>> state.is_draft
    True
    """

    tag: str
    is_draft: bool
    is_prerelease: bool
    assets: dict[str, str | None]


def _asset_digest(asset: cabc.Mapping[str, object]) -> str | None:
    """Return one asset's reported digest, or ``None`` when it is unusable.

    Returns
    -------
    str | None
        The digest GitHub reported, or ``None`` when the field is absent or
        not text. An absent digest is never coerced to an empty one, which
        would compare equal to nothing and read as a verified asset.

    Examples
    --------
    >>> _asset_digest({"digest": "sha256:ab"})
    'sha256:ab'
    >>> _asset_digest({"digest": 7}) is None
    True
    >>> _asset_digest({}) is None
    True
    """
    digest = asset.get("digest")
    return digest if isinstance(digest, str) else None


def _listed_assets(raw_assets: object) -> list[object]:
    """Require the response to carry the asset list GitHub documents.

    Returns
    -------
    list[object]
        The listed entries, still undecoded.

    Raises
    ------
    FinalizePolicyError
        If the response carries no list where the assets belong.

    Examples
    --------
    >>> _listed_assets([{"name": "a"}])
    [{'name': 'a'}]
    """
    if not isinstance(raw_assets, list):
        msg = "the release response does not carry an assets list"
        raise FinalizePolicyError(msg)
    return raw_assets


def _asset_object(asset: object) -> cabc.Mapping[str, object]:
    """Require one listed asset to be a JSON object.

    Returns
    -------
    cabc.Mapping[str, object]
        The entry, readable by field name.

    Raises
    ------
    FinalizePolicyError
        If the entry is not an object.
    """
    if not isinstance(asset, cabc.Mapping):
        msg = f"the release asset {asset!r} is not a JSON object"
        raise FinalizePolicyError(msg)
    return asset


def _asset_name(asset: cabc.Mapping[str, object]) -> str:
    """Require one asset object to name itself.

    Returns
    -------
    str
        The non-empty name the entry reported.

    Raises
    ------
    FinalizePolicyError
        If the entry carries no usable name.
    """
    name = asset.get("name")
    if not isinstance(name, str) or not name:
        msg = f"the release asset {asset!r} does not name an asset"
        raise FinalizePolicyError(msg)
    return name


def _asset_map(raw_assets: object) -> dict[str, str | None]:
    """Decode the response's asset list into a name-to-digest mapping.

    Returns
    -------
    dict[str, str | None]
        Every attached asset, in the order GitHub reported it.

    Raises
    ------
    FinalizePolicyError
        If the list is malformed, an entry is unnamed, or a name repeats.

    Notes
    -----
    A repeated name is refused because the two entries could carry different
    digests, and silently keeping the last would decide which one is the
    release's bytes without saying so.

    Examples
    --------
    >>> _asset_map([{"name": "a", "digest": "sha256:ab"}])
    {'a': 'sha256:ab'}
    """
    assets: dict[str, str | None] = {}
    for listed in _listed_assets(raw_assets):
        asset = _asset_object(listed)
        name = _asset_name(asset)
        if name in assets:
            msg = f"the release response names the asset {name!r} more than once"
            raise FinalizePolicyError(msg)
        assets[name] = _asset_digest(asset)
    return assets


def parse_release_state(payload: object, *, tag: str) -> ReleaseState:
    """Decode one ``gh release view`` response and check it names ``tag``.

    Parameters
    ----------
    payload
        The decoded JSON the command printed.
    tag
        The tag the command was asked about.

    Returns
    -------
    ReleaseState
        The release's draft flag, prerelease flag, and attached assets.

    Raises
    ------
    FinalizePolicyError
        If the response is not an object, names another tag, omits either
        boolean, or carries a malformed asset list.

    Notes
    -----
    The tag is checked rather than assumed: a response for another release
    would otherwise be read as this one's state, and the finalizer would
    un-draft whatever the API happened to return. The booleans are required,
    not defaulted, because ``False`` is a meaningful answer for both and a
    missing field must not silently become "not a draft".

    Examples
    --------
    >>> payload = {
    ...     "tagName": "v1.2.3",
    ...     "isDraft": True,
    ...     "isPrerelease": False,
    ...     "assets": [{"name": "a", "digest": "sha256:ab"}],
    ... }
    >>> parse_release_state(payload, tag="v1.2.3").assets
    {'a': 'sha256:ab'}
    """
    if not isinstance(payload, cabc.Mapping):
        msg = "the release response is not a JSON object"
        raise FinalizePolicyError(msg)
    reported_tag = payload.get("tagName")
    if reported_tag != tag:
        msg = f"the release response names tag {reported_tag!r}, not {tag!r}"
        raise FinalizePolicyError(msg)
    is_draft = payload.get("isDraft")
    is_prerelease = payload.get("isPrerelease")
    if not isinstance(is_draft, bool) or not isinstance(is_prerelease, bool):
        msg = "the release response must carry boolean draft and prerelease flags"
        raise FinalizePolicyError(msg)
    return ReleaseState(
        tag=tag,
        is_draft=is_draft,
        is_prerelease=is_prerelease,
        assets=_asset_map(payload.get("assets")),
    )


def is_already_finalized(state: ReleaseState, *, is_prerelease: bool) -> bool:
    """Return whether the release is already public with the wanted flag.

    Parameters
    ----------
    state
        The release as GitHub last reported it.
    is_prerelease
        The flag the validated candidate requires.

    Returns
    -------
    bool
        ``True`` when the release is public and its flag already agrees.

    Notes
    -----
    This is what makes an interrupted retry succeed without a second edit.
    Asking GitHub to un-draft an already-public release is harmless, but
    reporting the run as "finalized" when it made no call would misdescribe
    what happened; the distinction lets the step say "already finalized".

    Examples
    --------
    >>> public = ReleaseState("v1.2.3", False, True, {})
    >>> is_already_finalized(public, is_prerelease=True)
    True
    >>> is_already_finalized(public, is_prerelease=False)
    False
    >>> is_already_finalized(ReleaseState("v1.2.3", True, True, {}),
    ...                      is_prerelease=True)
    False
    """
    return not state.is_draft and state.is_prerelease is is_prerelease


def classify_readback(state: ReleaseState, *, is_prerelease: bool) -> Readback:
    """Classify a release after the un-drafting call.

    Parameters
    ----------
    state
        The release as GitHub reports it after the edit.
    is_prerelease
        The flag the validated candidate requires.

    Returns
    -------
    Readback
        ``FINALIZED`` when the release is public with the wanted flag,
        ``STILL_DRAFT`` when it remains a draft, and
        ``PRERELEASE_MISMATCH`` when it is public with the other flag.

    Notes
    -----
    The three outcomes get different diagnoses. A draft after a successful
    call means the API accepted the request but did not act on it, which is a
    provider fault; a public release with the wrong flag is a contradiction
    the release cannot resolve by retrying, because it is already public.

    Examples
    --------
    >>> classify_readback(ReleaseState("v1.2.3", False, False, {}),
    ...                   is_prerelease=False).value
    'finalized'
    >>> classify_readback(ReleaseState("v1.2.3", True, False, {}),
    ...                   is_prerelease=False).value
    'draft'
    >>> classify_readback(ReleaseState("v1.2.3", False, True, {}),
    ...                   is_prerelease=False).value
    'prerelease-mismatch'
    """
    if state.is_draft:
        return Readback.STILL_DRAFT
    if state.is_prerelease is is_prerelease:
        return Readback.FINALIZED
    return Readback.PRERELEASE_MISMATCH
