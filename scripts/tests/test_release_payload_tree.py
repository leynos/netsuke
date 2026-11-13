"""Test the tree adapter's payload index, provenance read, and install.

The adapter is where an existing output directory stops being a merge target
and where a copy either lands whole or leaves nothing behind, so these cases
build the shapes a real download can contain rather than asserting on names.
The per-file proof that bounds the copy lives in
``scripts/tests/test_release_payload_regular_files.py``.

Run via ``make test-release-publication``.
"""

import json
import os
import typing as typ

import pytest
import release_digest_files
import release_digest_record_files
import release_payload_selection
import release_payload_tree
from release_payload_errors import StagedFileMissingError
from release_payload_install_support import (
    selection_for,
    write_asset,
    write_provenance,
)
from release_payload_tree import RECORD_NAME

if typ.TYPE_CHECKING:
    import pathlib


def test_observe_payload_indexes_files_by_published_name(
    tmp_path: pathlib.Path,
) -> None:
    """Index one entry per name, with the platform rename already applied."""
    write_asset(tmp_path, "netsuke_1.0.0_amd64.deb")
    write_asset(tmp_path, "producer/netsuke-0.1.0~beta4-1.x86_64.rpm")
    assert sorted(release_payload_tree.observe_payload(tmp_path)) == [
        "netsuke-0.1.0.beta4-1.x86_64.rpm",
        "netsuke_1.0.0_amd64.deb",
    ], "the indexed names must carry the platform rename"


def test_observe_payload_does_not_index_producer_records(
    tmp_path: pathlib.Path,
) -> None:
    """Keep producer records out of the payload.

    A record left inside a producer directory is not a release asset, and a
    tree that indexed it would offer it for upload.
    """
    write_asset(tmp_path, "netsuke.deb")
    write_provenance(tmp_path / "producer", "linux-x86_64")
    assert sorted(release_payload_tree.observe_payload(tmp_path)) == ["netsuke.deb"], (
        "a producer record must never be indexed as an asset"
    )


def test_observe_payload_skips_symlinked_leaves(tmp_path: pathlib.Path) -> None:
    """Exclude a symlink rather than indexing what it points at."""
    real = write_asset(tmp_path / "real", "netsuke.deb")
    _ = (tmp_path / "link.deb").symlink_to(real)
    assert sorted(release_payload_tree.observe_payload(tmp_path)) == ["netsuke.deb"], (
        "the link must not appear beside the file it names"
    )


def test_observe_payload_propagates_an_unreadable_tree(
    tmp_path: pathlib.Path,
) -> None:
    """Surface a traversal failure rather than reporting an empty payload."""
    with pytest.raises(OSError, match="absent"):
        release_payload_tree.observe_payload(tmp_path / "absent")


def test_read_provenances_validates_and_sorts_by_target(
    tmp_path: pathlib.Path,
) -> None:
    """Read every record, sorted so refusals are stable between runs."""
    for target in ("zulu", "alpha"):
        directory = tmp_path / target
        directory.mkdir()
        write_provenance(directory, target)
    targets = [
        record.target for record in release_payload_tree.read_provenances(tmp_path)
    ]
    assert targets == ["alpha", "zulu"], "the records must be read in sorted order"


def test_read_provenances_reports_an_unusable_record(tmp_path: pathlib.Path) -> None:
    """Surface the JSON reader's own diagnosis for a malformed record."""
    _ = (tmp_path / RECORD_NAME).write_text("not json", encoding="utf-8")
    with pytest.raises(json.JSONDecodeError):
        release_payload_tree.read_provenances(tmp_path)


def test_a_producer_record_that_is_not_a_regular_file_is_never_read(
    tmp_path: pathlib.Path,
) -> None:
    """Skip a staged entry named like a record that is not one.

    A FIFO in a downloaded tree would block ``read_text`` waiting for a writer,
    which a release job cannot distinguish from a slow disk. The name alone
    must not be enough to reach the reader.
    """
    os.mkfifo(tmp_path / RECORD_NAME)

    assert release_payload_tree.read_provenances(tmp_path) == [], (
        "a non-regular entry must be skipped, not read"
    )
    assert release_payload_tree.read_provenance_by_path(tmp_path) == {}, (
        "the by-path view must apply the same regularity rule"
    )


def test_copy_verified_copies_the_bytes(tmp_path: pathlib.Path) -> None:
    """Copy a proven regular file."""
    source = write_asset(tmp_path, "a.deb", b"payload")
    destination = tmp_path / "out" / "a.deb"
    destination.parent.mkdir()
    release_payload_tree.copy_verified(source, destination, staged_root=tmp_path)
    assert destination.read_bytes() == b"payload", "the bytes must be copied verbatim"


def test_copy_verified_refuses_a_symlink(tmp_path: pathlib.Path) -> None:
    """Refuse to copy through a link."""
    real = write_asset(tmp_path / "real", "a.deb")
    link = tmp_path / "link.deb"
    _ = link.symlink_to(real)
    destination = tmp_path / "out.deb"
    with pytest.raises(StagedFileMissingError):
        release_payload_tree.copy_verified(link, destination, staged_root=tmp_path)
    assert not destination.exists(), "a refused copy must leave nothing behind"


def test_install_payload_copies_and_writes_the_record(
    tmp_path: pathlib.Path,
) -> None:
    """Install the payload and the record of what selection approved."""
    dist = tmp_path / "dist"
    source = write_asset(dist, "a.deb", b"payload")
    output = tmp_path / "artifacts"
    record = tmp_path / "record.json"
    selection = selection_for({"a.deb": source})
    release_payload_tree.install_payload(selection, output, record, dist)
    installed = output / release_payload_tree.PAYLOAD_DIRECTORY / "a.deb"
    assert installed.read_bytes() == b"payload", "the payload must be installed"
    assert (
        release_digest_record_files.read_digest_record(record).as_mapping()
        == selection.trusted
    ), "the record must carry the digests selection was approved against"


def test_install_payload_records_the_approved_digests_not_a_fresh_hash(
    tmp_path: pathlib.Path,
) -> None:
    """Keep the approved digests, so a swap after the check stays visible.

    A record re-hashed from the copy would agree with whatever bytes happened
    to be copied, and the uploader's comparison against it would then bless the
    substitution instead of catching it.
    """
    dist = tmp_path / "dist"
    source = write_asset(dist, "a.deb", b"payload")
    output = tmp_path / "artifacts"
    record = tmp_path / "record.json"
    approved = {"a.deb": release_digest_files.digest_of_file(source)}
    _ = source.write_bytes(b"swapped between the digest check and the copy")
    release_payload_tree.install_payload(
        release_payload_selection.PayloadSelection(
            declared=("a.deb",), trusted=approved, sources={"a.deb": source}
        ),
        output,
        record,
        dist,
    )
    installed = output / release_payload_tree.PAYLOAD_DIRECTORY / "a.deb"
    assert installed.read_bytes() == b"swapped between the digest check and the copy", (
        "the copy reads whatever the source holds when it runs"
    )
    assert (
        release_digest_record_files.read_digest_record(record).as_mapping() == approved
    ), "the record must keep the approved digests so the swap stays visible"


def test_install_payload_refuses_an_existing_output_directory(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse to merge into an output directory that already exists.

    A stale directory from an earlier attempt would otherwise be mistaken for
    this run's payload.
    """
    dist = tmp_path / "dist"
    source = write_asset(dist, "a.deb")
    output = tmp_path / "artifacts"
    output.mkdir()
    with pytest.raises(FileExistsError):
        release_payload_tree.install_payload(
            selection_for({"a.deb": source}), output, tmp_path / "record.json", dist
        )


def test_install_payload_removes_a_partial_directory_on_failure(
    tmp_path: pathlib.Path,
) -> None:
    """Leave no partial payload behind when a copy fails."""
    dist = tmp_path / "dist"
    good = write_asset(dist, "a.deb")
    output = tmp_path / "artifacts"
    with pytest.raises(StagedFileMissingError):
        release_payload_tree.install_payload(
            selection_for({"a.deb": good, "absent.deb": dist / "absent.deb"}),
            output,
            tmp_path / "record.json",
            dist,
        )
    assert not output.exists(), "a failed install must not leave a payload to upload"


def test_install_payload_refuses_a_source_outside_the_staged_root(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a source that is not below the tree the producers staged.

    The selection's own names came from that tree, so a source outside it is a
    different claim from the one selection made, and the copy must not make it.
    """
    dist = tmp_path / "dist"
    dist.mkdir()
    source = write_asset(tmp_path / "elsewhere", "a.deb")
    output = tmp_path / "artifacts"
    with pytest.raises(StagedFileMissingError, match="inside the staged tree"):
        release_payload_tree.install_payload(
            selection_for({"a.deb": source}), output, tmp_path / "record.json", dist
        )
    assert not output.exists(), "a refused install must not leave a payload to upload"
