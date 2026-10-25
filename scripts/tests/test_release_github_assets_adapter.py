"""Check GitHub's release-asset spelling at the filesystem boundary."""

import pytest
import release_github_assets_adapter


@pytest.mark.parametrize(
    ("staged", "published"),
    [
        ("netsuke-0.1.0~beta4-1.x86_64.rpm", "netsuke-0.1.0.beta4-1.x86_64.rpm"),
        ("netsuke_0.1.0~beta4-1_arm64.deb", "netsuke_0.1.0.beta4-1_arm64.deb"),
        (
            "netsuke-0.1.0-beta4-macos-arm64.pkg",
            "netsuke-0.1.0-beta4-macos-arm64.pkg",
        ),
        ("netsuke-0.1.0-beta4-x64.msi", "netsuke-0.1.0-beta4-x64.msi"),
    ],
)
def test_the_adapter_maps_only_github_rewritten_characters(
    staged: str, published: str
) -> None:
    """Map the tilde and leave every other character unchanged."""
    assert release_github_assets_adapter.published_asset_name(staged) == published, (
        "the adapter must match GitHub's published asset spelling"
    )
