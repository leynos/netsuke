"""Hold the uploaded payload to the exact paths the record describes.

Two claims are checked here, and they are checked separately because they fail
differently. The set claim -- does this directory hold the declared names --
is about what a job was handed. The leaf claim -- is ``payload/a.deb`` the file
``gh`` will receive -- is about *which* entry that name resolves to, and a tree
can satisfy the first while failing the second.

The cases build the shapes a restored artefact can really contain rather than
asserting on message text: a nested duplicate, a symlink beside a regular file
of the same published name, and an appended file. Each is a hand-off that
would otherwise publish bytes nobody selected.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_digest_files
import release_digest_record
import release_payload_handoff
from release_payload_install_support import write_asset, write_provenance

if typ.TYPE_CHECKING:
    import pathlib


def test_require_payload_matches_accepts_the_installed_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a payload directory holding exactly the recorded bytes."""
    asset = write_asset(tmp_path, "a.deb", b"payload")
    digest = release_digest_files.digest_of_file(asset)
    release_payload_handoff.require_payload_matches(tmp_path, {"a.deb": digest})


def test_require_payload_matches_names_what_the_payload_omits(
    tmp_path: pathlib.Path,
) -> None:
    """Report a hand-off that lost a declared asset."""
    asset = write_asset(tmp_path, "a.deb")
    digest = release_digest_files.digest_of_file(asset)
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="omits"):
        release_payload_handoff.require_payload_matches(
            tmp_path, {"a.deb": digest, "b.rpm": "b" * 64}
        )


def test_require_payload_matches_names_what_the_payload_adds(
    tmp_path: pathlib.Path,
) -> None:
    """Report a hand-off that gained a file nobody selected."""
    asset = write_asset(tmp_path, "a.deb")
    write_asset(tmp_path, "extra.rpm")
    digest = release_digest_files.digest_of_file(asset)
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="adds"):
        release_payload_handoff.require_payload_matches(tmp_path, {"a.deb": digest})


def test_require_payload_matches_refuses_replaced_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a payload whose bytes are not the recorded ones."""
    write_asset(tmp_path, "a.deb", b"tampered")
    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        release_payload_handoff.require_payload_matches(tmp_path, {"a.deb": "b" * 64})


def test_a_producer_record_is_not_a_payload_asset(tmp_path: pathlib.Path) -> None:
    """Keep the hand-off check from accepting a record as an asset."""
    write_provenance(tmp_path, "linux-x86_64")
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="omits"):
        release_payload_handoff.require_payload_matches(tmp_path, {"a.deb": "b" * 64})


def test_a_nested_duplicate_of_a_declared_name_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a second file publishing under a name the release declares.

    The nested copy is a regular file with the trusted digest, so hashing
    ``present[name][0]`` would have accepted it while the root-level entry the
    uploader sends went unread.
    """
    asset = write_asset(tmp_path, "a.deb", b"payload")
    write_asset(tmp_path / "sub", "a.deb", b"payload")
    digest = release_digest_files.digest_of_file(asset)
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="adds"):
        release_payload_handoff.require_payload_matches(tmp_path, {"a.deb": digest})


def test_a_root_symlink_is_refused_beside_a_regular_nested_file(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse the symlink the uploader would send, not the copy that vouches.

    ``observe_payload`` skips the link and indexes only ``sub/a.deb``, which
    is a regular file holding the trusted digest. The uploader sends
    ``payload/a.deb``, which is the link, and that resolves outside the
    payload entirely. The link target is deliberately *outside* the payload
    directory: had it been inside, the duplicate-name check would refuse the
    tree for a different reason and this case would prove nothing about which
    path is hashed.
    """
    payload = tmp_path / "payload"
    elsewhere = write_asset(tmp_path / "elsewhere", "a.deb", b"not the payload")
    asset = write_asset(payload / "sub", "a.deb", b"payload")
    _ = (payload / "a.deb").symlink_to(elsewhere)
    digest = release_digest_files.digest_of_file(asset)
    with pytest.raises(
        release_payload_handoff.PayloadHandoffError, match="cannot upload"
    ):
        release_payload_handoff.require_payload_matches(payload, {"a.deb": digest})
