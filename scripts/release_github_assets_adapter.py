"""Translate staged names into GitHub release asset names.

GitHub changes one character in the package asset names this workflow uploads:
it publishes ``~`` as ``.``. This adapter owns that provider-specific spelling
so the declaration policy can operate only on names it is given, while the
staged-digest reader can still associate a published name with its source file.

Run through the declaration CLI and staged-digest collector; behavioural
coverage lives in ``scripts/tests/test_release_github_assets_adapter.py``.
"""

#: Characters GitHub rewrites in this repository's asset names.
PUBLISHED_CHARACTERS = {"~": "."}


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
