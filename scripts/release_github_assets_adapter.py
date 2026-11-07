"""Translate GitHub's release-asset spelling and digest format at the boundary.

GitHub changes one character in the package asset names this workflow uploads:
it publishes ``~`` as ``.``. This adapter owns that provider-specific spelling
so the declaration policy can operate only on names it is given, while the
staged-digest reader can still associate a published name with its source file.

It also owns the digest *representation*: GitHub reports ``sha256:<hex>`` while
the trusted record stores bare hexadecimal. Both the upload reconciliation and
the finalizer need that conversion, so it lives here rather than being copied
into each CLI -- a second copy is a second place for the two to disagree about
what an unusable digest is.

Run through the declaration CLI, staged-digest collector, uploader, and
finalizer; behavioural coverage lives in
``scripts/tests/test_release_github_assets_adapter.py``.
"""

#: Characters GitHub rewrites in this repository's asset names.
PUBLISHED_CHARACTERS = {"~": "."}

#: Prefix GitHub puts on the digest it reports for an asset.
GITHUB_DIGEST_PREFIX = "sha256:"


def published_asset_name(staged_name: str) -> str:
    """Return the name GitHub publishes for a staged asset.

    Returns
    -------
    str
        The GitHub-visible asset name.

    Examples
    --------
    >>> published_asset_name("netsuke-0.1.0~beta4-1.x86_64.rpm")
    'netsuke-0.1.0.beta4-1.x86_64.rpm'
    >>> published_asset_name("netsuke-0.1.0-beta4-macos-arm64.pkg")
    'netsuke-0.1.0-beta4-macos-arm64.pkg'
    """
    published = staged_name
    for staged, replacement in PUBLISHED_CHARACTERS.items():
        published = published.replace(staged, replacement)
    return published


def normalise_github_digest(reported: object) -> str | None:
    """Convert GitHub's vendor digest into the policy's bare digest value.

    Parameters
    ----------
    reported
        The ``digest`` value from a ``gh release view --json assets`` entry.

    Returns
    -------
    str | None
        The bare lowercase digest, or ``None`` when GitHub's value is unusable.

    Notes
    -----
    ``None`` is a distinct answer from any digest: it says the provider
    supplied nothing this release can compare against, which reconciliation
    treats as a conflict rather than a match. Keeping that distinction here,
    rather than folding an unusable value into an empty string, is what stops
    "nothing verifiable" from reading as "nothing wrong".

    Examples
    --------
    >>> normalise_github_digest("sha256:" + "AB" * 32) == "ab" * 32
    True
    >>> normalise_github_digest("md5:abcd") is None
    True
    >>> normalise_github_digest(None) is None
    True
    """
    if not isinstance(reported, str) or not reported.startswith(GITHUB_DIGEST_PREFIX):
        return None
    digest = reported.removeprefix(GITHUB_DIGEST_PREFIX).strip().lower()
    return digest or None
