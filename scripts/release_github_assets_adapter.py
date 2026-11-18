"""Translate GitHub's release-asset spelling and digest format at the boundary.

GitHub changes one character in the package asset names this workflow uploads:
it publishes ``~`` as ``.``. This adapter owns that provider-specific spelling
so the declaration policy can operate only on names it is given, while the
staged-digest reader can still associate a published name with its source file.

It also owns the digest *representation*: GitHub reports ``sha256:<hex>`` while
the trusted record stores bare hexadecimal. Both the upload reconciliation and
the release finalizer need that conversion, so it lives here rather than being
copied into each CLI -- a second copy is a second place for the two to disagree
about what an unusable digest is.

The release workflow runs the declaration CLI, the staged-digest collector, the
uploader, and the finalizer. The finalizer converts twice for one run: the
planner normalises the snapshot it was given, and `publish` normalises the
fresh snapshot it takes immediately before the edit.

Run through the declaration CLI, staged-digest collector, uploader, and
finalizer; behavioural coverage lives in
``scripts/tests/test_release_github_assets_adapter.py``.
"""

#: Characters GitHub rewrites in this repository's asset names.
PUBLISHED_CHARACTERS = {"~": "."}

#: Prefix GitHub puts on the digest it reports for an asset.
GITHUB_DIGEST_PREFIX = "sha256:"

#: Where a release asset is downloadable without a credential. The URL
#: redirects to a signed object URL on GitHub's asset host; both hops are
#: HTTPS, so a consumer following the redirect never downgrades.
RELEASE_DOWNLOAD_BASE = "https://github.com"


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


def public_download_url(repository: str, tag: str, asset_name: str) -> str:
    """Return the URL any consumer uses to download one release asset.

    Parameters
    ----------
    repository
        The ``owner/name`` slug the release was published under.
    tag
        The release tag, e.g. ``v1.2.3``.
    asset_name
        The name GitHub publishes, which
        :func:`published_asset_name` derives from the staged one.

    Returns
    -------
    str
        The public HTTPS URL of the asset.

    Notes
    -----
    This is the same URL a browser follows, so the smoke exercises the path a
    consumer takes rather than an API route only an authenticated client uses.
    No escaping is applied: every caller has already required
    ``asset_name`` to be one bare file name, and a name needing escaping is
    one this workflow would not have published.

    Examples
    --------
    >>> public_download_url("leynos/netsuke", "v1.2.3", "netsuke-1.2.3.tar.gz")
    'https://github.com/leynos/netsuke/releases/download/v1.2.3/netsuke-1.2.3.tar.gz'
    """
    return f"{RELEASE_DOWNLOAD_BASE}/{repository}/releases/download/{tag}/{asset_name}"


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
    reports as unverifiable rather than as a match. Keeping that distinction
    here, rather than folding an unusable value into an empty string, is what
    stops "nothing verifiable" from reading as "nothing wrong".

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
