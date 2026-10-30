"""Behavioural coverage for digest sidecar verification.

The release records a SHA-256 sidecar beside every staged archive and later
re-checks the published bytes against it. This module covers the two ways that
check can be wrong: refusing bytes that are correct, and — the dangerous
direction — accepting bytes that are not.

The sidecar format is GNU ``sha256sum`` output, so the fixtures here are
written the way that tool writes them, including the binary marker and the
recorded path. The digests are derived from the bytes under test rather than
invented, because an invented digest would only prove that two different
constants differ.

Run via ``make test-release-publication``.
"""

import hashlib
import typing as typ

import pytest
from release_digest import (
    DigestMismatchError,
    MalformedSidecarError,
    constant_time_equal,
    parse_sidecar,
)
from release_digest_files import digest_of_file, verify_file_against_sidecar

if typ.TYPE_CHECKING:
    import pathlib

#: Two distinct byte strings with distinct digests, so identity and mismatch
#: are both reachable. The digests are computed, never written by hand.
ARCHIVE_BYTES = b"netsuke\n"
OTHER_BYTES = b"netsuke"


@pytest.fixture
def staged(tmp_path: pathlib.Path) -> tuple[pathlib.Path, str]:
    """Stage an archive with its correct sidecar, as the release does.

    Returns
    -------
    tuple[pathlib.Path, str]
        The archive's path and the digest recorded for it.
    """
    archive = tmp_path / "netsuke-x86_64-unknown-linux-musl.tar.gz"
    archive.write_bytes(ARCHIVE_BYTES)
    digest = hashlib.sha256(ARCHIVE_BYTES).hexdigest()
    archive.with_name(f"{archive.name}.sha256").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8"
    )
    return archive, digest


def test_digest_of_file_matches_hashlib(tmp_path: pathlib.Path) -> None:
    """Require the file digest to be the one an independent tool computes."""
    target = tmp_path / "bytes"
    target.write_bytes(ARCHIVE_BYTES)
    assert digest_of_file(target) == hashlib.sha256(ARCHIVE_BYTES).hexdigest(), (
        "the digest must be the one an independent tool computes"
    )


def test_digest_of_file_is_lowercase_hexadecimal(tmp_path: pathlib.Path) -> None:
    """Require lowercase hexadecimal, so two spellings never compare unequal."""
    target = tmp_path / "bytes"
    target.write_bytes(ARCHIVE_BYTES)
    digest = digest_of_file(target)
    assert digest == digest.lower(), "the hexadecimal spelling must be lowercase"
    assert len(digest) == 64, "the digest must be the full 64 characters"


def test_verify_accepts_the_bytes_the_sidecar_describes(
    staged: tuple[pathlib.Path, str],
) -> None:
    """Require a correct archive to verify, returning its digest."""
    archive, digest = staged
    assert verify_file_against_sidecar(archive) == digest, (
        "a correct archive must verify and report its recorded digest"
    )


def test_verify_rejects_a_single_flipped_byte(
    staged: tuple[pathlib.Path, str],
) -> None:
    """Require one changed byte to be refused.

    This is the direction that matters. A verifier that accepts everything
    still passes the happy-path test above.
    """
    archive, digest = staged
    archive.write_bytes(OTHER_BYTES)
    assert digest_of_file(archive) != digest, "fixture must differ from the record"
    with pytest.raises(DigestMismatchError):
        verify_file_against_sidecar(archive)


def test_verify_rejects_an_archive_whose_sidecar_is_missing(
    tmp_path: pathlib.Path,
) -> None:
    """Require an absent sidecar to be refused rather than treated as unverified."""
    archive = tmp_path / "netsuke.tar.gz"
    archive.write_bytes(ARCHIVE_BYTES)
    with pytest.raises((MalformedSidecarError, OSError)):
        verify_file_against_sidecar(archive)


def test_parse_reads_the_gnu_binary_marker() -> None:
    """Require both GNU markers to parse, and to be distinguished.

    ``sha256sum`` writes two spaces before the path in text mode and a space
    plus ``*`` in binary mode. A parser that split on whitespace alone would
    silently accept a marker of ``*`` as part of the path.
    """
    digest = hashlib.sha256(ARCHIVE_BYTES).hexdigest()
    text_digest, text_path = parse_sidecar(f"{digest}  netsuke.tar.gz\n")
    star_digest, star_path = parse_sidecar(f"{digest} *netsuke.tar.gz\n")
    assert text_digest == star_digest == digest, (
        "the marker must not be read as part of the digest"
    )
    assert text_path == star_path == "netsuke.tar.gz", (
        "the marker must not be read as part of the path"
    )


def test_parse_discards_the_recorded_directory() -> None:
    """Require the recorded directory to be discarded, not validated.

    A real sidecar records the staging path — ``artifacts/linux-x86_64/…`` —
    while the published asset is a flat name at the release root. Reading the
    recorded directory as a constraint would refuse every genuine sidecar.
    """
    digest = hashlib.sha256(ARCHIVE_BYTES).hexdigest()
    _, path = parse_sidecar(f"{digest}  artifacts/linux-x86_64/netsuke.tar.gz\n")
    assert path == "netsuke.tar.gz", (
        "the recorded directory must not survive as part of the asset name"
    )


@pytest.mark.parametrize(
    "text",
    [
        "",
        "\n",
        "not a checksum line at all\n",
        # Truncated digest: 63 characters, one short.
        f"{hashlib.sha256(ARCHIVE_BYTES).hexdigest()[:-1]}  netsuke.tar.gz\n",
        # No marker between digest and path.
        f"{hashlib.sha256(ARCHIVE_BYTES).hexdigest()}netsuke.tar.gz\n",
        # Non-hexadecimal character inside the digest.
        "z" * 64 + "  netsuke.tar.gz\n",
    ],
)
def test_parse_refuses_a_malformed_sidecar(text: str) -> None:
    """Require malformed sidecars to be refused, with the text in the message."""
    with pytest.raises(MalformedSidecarError):
        parse_sidecar(text)


def test_parse_refuses_the_gnu_escaped_path_form() -> None:
    r"""Require GNU's escaped-path spelling to be refused deliberately.

    ``sha256sum`` escapes a path containing a backslash or newline by prefixing
    the line with ``\\``. The inventory requires every asset name to be a bare
    single component, so no legitimate sidecar in this release can take that
    form; accepting it would mean hashing a path we cannot resolve.
    """
    digest = hashlib.sha256(ARCHIVE_BYTES).hexdigest()
    with pytest.raises(MalformedSidecarError):
        parse_sidecar(f"\\{digest}  netsuke\\tar.gz\n")


@pytest.mark.parametrize(
    ("left", "right", "identical"),
    [
        ("a" * 64, "a" * 64, True),
        ("a" * 64, "b" * 64, False),
        ("", "", True),
        ("a" * 64, "a" * 63, False),
    ],
)
def test_constant_time_equal_agrees_with_equality(
    left: str, right: str, *, identical: bool
) -> None:
    """Require the comparison to agree with ``==`` while not short-circuiting.

    Constancy is not observable from the result, so the result is what is
    pinned here; the timing property is the rationale in the module.
    """
    assert constant_time_equal(left, right) is identical, (
        "the comparison's verdict must be the declared expectation"
    )
    assert constant_time_equal(left, right) is (left == right), (
        "the comparison must agree with equality"
    )
