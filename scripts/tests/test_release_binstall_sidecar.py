"""Test verifying the published checksum sidecar of a downloaded archive.

The sidecar is what a consumer's installer reads, so it is judged separately
from the archive's own trusted digest. The fixtures come from
:mod:`release_binstall_fixtures`, which also serves the archive, extraction,
and probe suites.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_binstall_sidecar as sidecar_adapter
from release_binstall_fixtures import SIDECAR_NAME, digest_of, tarball

if typ.TYPE_CHECKING:
    import pathlib


def test_verify_sidecar_accepts_a_matching_checksum(tmp_path: pathlib.Path) -> None:
    """Return the sidecar's digest when it describes the archive."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    digest = digest_of(archive)
    _ = sidecar.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")

    assert sidecar_adapter.verify_sidecar(archive, sidecar) == digest, (
        "a matching sidecar must return the archive's digest"
    )


@pytest.mark.parametrize(
    "text",
    [
        "",
        "not a checksum\n",
        "cd" * 32 + "  archive.tar.gz\n",
    ],
    ids=["empty", "malformed", "other-digest"],
)
def test_verify_sidecar_refuses_a_sidecar_that_does_not_describe_the_archive(
    tmp_path: pathlib.Path, text: str
) -> None:
    """Refuse a sidecar that is empty, malformed, or names other bytes."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_text(text, encoding="utf-8")

    with pytest.raises(sidecar_adapter.SidecarVerificationError):
        sidecar_adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_a_sidecar_naming_another_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a sidecar whose checksum is right but which names another asset.

    The digest here is the archive's own, so only the asset name can produce
    the refusal; a case that also changed the digest would stay green if the
    name check were deleted, because the digest check would raise instead.
    """
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_text(
        f"{digest_of(archive)}  another-asset.tar.gz\n", encoding="utf-8"
    )

    with pytest.raises(sidecar_adapter.SidecarVerificationError):
        sidecar_adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_an_oversized_body(tmp_path: pathlib.Path) -> None:
    """Refuse a sidecar larger than one checksum line."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_bytes(b"a" * (sidecar_adapter.MAX_SIDECAR_BYTES + 1))

    with pytest.raises(sidecar_adapter.SidecarVerificationError):
        sidecar_adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_a_non_utf8_body(tmp_path: pathlib.Path) -> None:
    """Refuse bytes that are not text at all."""
    archive = tarball(tmp_path / "archive.tar.gz")
    sidecar = tmp_path / SIDECAR_NAME
    _ = sidecar.write_bytes(b"\xff\xfe\x00")

    with pytest.raises(sidecar_adapter.SidecarVerificationError):
        sidecar_adapter.verify_sidecar(archive, sidecar)


def test_verify_sidecar_refuses_an_absent_sidecar(tmp_path: pathlib.Path) -> None:
    """Refuse a download that never arrived."""
    archive = tarball(tmp_path / "archive.tar.gz")

    with pytest.raises(sidecar_adapter.SidecarVerificationError):
        sidecar_adapter.verify_sidecar(archive, tmp_path / SIDECAR_NAME)
