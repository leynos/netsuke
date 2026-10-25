"""Exercise the artifact-file adapter around pure digest policy."""

import hashlib
import typing as typ

import pytest
import release_digest
from release_digest_files import digest_of_file, verify_file_against_sidecar

if typ.TYPE_CHECKING:
    import pathlib


def test_file_digest_matches_hashlib(tmp_path: pathlib.Path) -> None:
    """Hash the file in the adapter and compare it with an independent digest."""
    asset = tmp_path / "asset.bin"
    _ = asset.write_bytes(b"release bytes")

    assert digest_of_file(asset) == hashlib.sha256(b"release bytes").hexdigest(), (
        "the file adapter must hash the complete file contents"
    )


def test_sidecar_adapter_reads_text_and_delegates_policy(
    tmp_path: pathlib.Path,
) -> None:
    """Read the neighbouring sidecar and return the verified file digest."""
    asset = tmp_path / "asset.bin"
    _ = asset.write_bytes(b"release bytes")
    digest = hashlib.sha256(b"release bytes").hexdigest()
    _ = asset.with_name("asset.bin.sha256").write_text(
        f"{digest}  artifacts/asset.bin\n", encoding="utf-8"
    )

    assert verify_file_against_sidecar(asset) == digest, (
        "the adapter must return the digest accepted by sidecar policy"
    )


def test_sidecar_adapter_preserves_policy_mismatch_error(
    tmp_path: pathlib.Path,
) -> None:
    """Keep a digest disagreement in the policy error hierarchy."""
    asset = tmp_path / "asset.bin"
    _ = asset.write_bytes(b"release bytes")
    _ = asset.with_name("asset.bin.sha256").write_text(
        f"{'0' * 64}  asset.bin\n", encoding="utf-8"
    )

    with pytest.raises(release_digest.DigestMismatchError):
        verify_file_against_sidecar(asset)
