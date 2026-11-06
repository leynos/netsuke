"""Test the filesystem boundary the payload selection reads and writes through.

The tree adapter is where a symlink stops being an ordinary file and where an
existing output directory stops being a merge target, so these cases build the
hostile shapes a real download can contain rather than asserting on names.

Run via ``make test-release-publication``.
"""

import json
import os
import stat
import typing as typ

import pytest
import release_digest_files
import release_digest_record
import release_digest_record_files
import release_payload_selection
import release_payload_tree
from release_payload_errors import StagedFileMissingError
from release_payload_tree import RECORD_NAME

if typ.TYPE_CHECKING:
    import pathlib


def write_asset(
    directory: pathlib.Path, name: str, content: bytes = b"bytes"
) -> pathlib.Path:
    """Write ``content`` under ``name`` and return the path.

    Returns
    -------
    pathlib.Path
        The written file's path.
    """
    path = directory / name
    path.parent.mkdir(parents=True, exist_ok=True)
    _ = path.write_bytes(content)
    return path


def write_provenance(directory: pathlib.Path, target: str) -> None:
    """Write a valid producer record for ``target`` below ``directory``."""
    record = {
        "schema": 1,
        "commit": "a" * 40,
        "run_id": "7",
        "run_attempt": "1",
        "target": target,
        "platform": "linux",
        "assets": {"a.deb": "b" * 64},
    }
    directory.mkdir(parents=True, exist_ok=True)
    _ = (directory / RECORD_NAME).write_text(json.dumps(record), encoding="utf-8")


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


def test_require_regular_accepts_an_ordinary_file(tmp_path: pathlib.Path) -> None:
    """Accept a regular file below regular directories."""
    asset = write_asset(tmp_path, "a.deb")
    assert release_payload_tree.require_regular(asset) == asset, (
        "an ordinary file must be returned unchanged"
    )


def test_require_regular_refuses_a_symlinked_leaf(tmp_path: pathlib.Path) -> None:
    """Refuse a leaf that is a link, judged without following it."""
    real = write_asset(tmp_path / "real", "a.deb")
    link = tmp_path / "link.deb"
    _ = link.symlink_to(real)
    with pytest.raises(StagedFileMissingError, match="regular file"):
        release_payload_tree.require_regular(link)


def test_require_regular_refuses_a_symlinked_ancestor(tmp_path: pathlib.Path) -> None:
    """Refuse a file reached through a linked directory.

    Following it would let the payload hold bytes from outside the downloaded
    tree, which is the substitution the trusted record exists to catch.
    """
    real_dir = tmp_path / "elsewhere"
    asset = write_asset(real_dir, "a.deb")
    linked_dir = tmp_path / "producer"
    _ = linked_dir.symlink_to(real_dir, target_is_directory=True)
    with pytest.raises(StagedFileMissingError, match="directory"):
        release_payload_tree.require_regular(linked_dir / asset.name)


@pytest.mark.parametrize(
    ("mode", "expected"),
    [(stat.S_IFDIR, "directory"), (stat.S_IFIFO, "regular file")],
)
def test_require_regular_refuses_non_file_modes(
    tmp_path: pathlib.Path, *, mode: int, expected: str
) -> None:
    """Refuse every non-regular mode, not only links.

    A FIFO in a downloaded tree would block a copy indefinitely, which a release
    job cannot distinguish from a slow transfer.
    """
    if mode == stat.S_IFDIR:
        candidate = tmp_path / "a-directory"
        candidate.mkdir()
        with pytest.raises(StagedFileMissingError, match="directory"):
            release_payload_tree.require_regular(candidate / "a.deb")
        return
    fifo = tmp_path / "a-fifo"
    os.mkfifo(fifo)
    with pytest.raises(StagedFileMissingError, match=expected):
        release_payload_tree.require_regular(fifo)


def test_require_regular_refuses_a_missing_file(tmp_path: pathlib.Path) -> None:
    """Surface the absent path rather than a bare ``FileNotFoundError``."""
    with pytest.raises(StagedFileMissingError, match="regular file"):
        release_payload_tree.require_regular(tmp_path / "absent.deb")


def test_copy_verified_copies_the_bytes(tmp_path: pathlib.Path) -> None:
    """Copy a proven regular file."""
    source = write_asset(tmp_path, "a.deb", b"payload")
    destination = tmp_path / "out" / "a.deb"
    destination.parent.mkdir()
    release_payload_tree.copy_verified(source, destination)
    assert destination.read_bytes() == b"payload", "the bytes must be copied verbatim"


def test_copy_verified_refuses_a_symlink(tmp_path: pathlib.Path) -> None:
    """Refuse to copy through a link."""
    real = write_asset(tmp_path / "real", "a.deb")
    link = tmp_path / "link.deb"
    _ = link.symlink_to(real)
    destination = tmp_path / "out.deb"
    with pytest.raises(StagedFileMissingError):
        release_payload_tree.copy_verified(link, destination)
    assert not destination.exists(), "a refused copy must leave nothing behind"


def selection_for(
    sources: dict[str, pathlib.Path],
) -> release_payload_selection.PayloadSelection[pathlib.Path]:
    """Name ``sources`` as the approved payload.

    Returns
    -------
    release_payload_selection.PayloadSelection
        A selection whose declared names are the source keys.
    """
    return release_payload_selection.PayloadSelection(
        declared=tuple(sorted(sources)),
        trusted=dict.fromkeys(sources, "b" * 64),
        sources=sources,
    )


def test_install_payload_copies_and_writes_the_record(
    tmp_path: pathlib.Path,
) -> None:
    """Install the payload and its trusted record in one step."""
    source = write_asset(tmp_path / "dist", "a.deb", b"payload")
    output = tmp_path / "artifacts"
    record = tmp_path / "record.json"
    computed = {"a.deb": release_digest_files.digest_of_file(source)}
    release_payload_tree.install_payload(
        selection_for({"a.deb": source}), output, record, computed
    )
    installed = output / release_payload_tree.PAYLOAD_DIRECTORY / "a.deb"
    assert installed.read_bytes() == b"payload", "the payload must be installed"
    assert release_digest_record_files.read_digest_record(record) == computed, (
        "the record must carry the digest the caller computed"
    )


def test_install_payload_refuses_an_existing_output_directory(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse to merge into an output directory that already exists.

    A stale directory from an earlier attempt would otherwise be mistaken for
    this run's payload.
    """
    source = write_asset(tmp_path / "dist", "a.deb")
    output = tmp_path / "artifacts"
    output.mkdir()
    with pytest.raises(FileExistsError):
        release_payload_tree.install_payload(
            selection_for({"a.deb": source}),
            output,
            tmp_path / "record.json",
            {"a.deb": "b" * 64},
        )


def test_install_payload_removes_a_partial_directory_on_failure(
    tmp_path: pathlib.Path,
) -> None:
    """Leave no partial payload behind when a copy fails."""
    good = write_asset(tmp_path / "dist", "a.deb")
    output = tmp_path / "artifacts"
    with pytest.raises(StagedFileMissingError):
        release_payload_tree.install_payload(
            selection_for({
                "a.deb": good,
                "absent.deb": tmp_path / "dist" / "absent.deb",
            }),
            output,
            tmp_path / "record.json",
            {"a.deb": "b" * 64},
        )
    assert not output.exists(), "a failed install must not leave a payload to upload"


def test_require_payload_matches_accepts_the_installed_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a payload directory holding exactly the recorded bytes."""
    asset = write_asset(tmp_path, "a.deb", b"payload")
    digest = release_digest_files.digest_of_file(asset)
    release_payload_tree.require_payload_matches(tmp_path, {"a.deb": digest})


def test_require_payload_matches_names_what_the_payload_omits(
    tmp_path: pathlib.Path,
) -> None:
    """Report a hand-off that lost a declared asset."""
    asset = write_asset(tmp_path, "a.deb")
    digest = release_digest_files.digest_of_file(asset)
    with pytest.raises(release_payload_tree.PayloadHandoffError, match="omits"):
        release_payload_tree.require_payload_matches(
            tmp_path, {"a.deb": digest, "b.rpm": "b" * 64}
        )


def test_require_payload_matches_names_what_the_payload_adds(
    tmp_path: pathlib.Path,
) -> None:
    """Report a hand-off that gained a file nobody selected."""
    asset = write_asset(tmp_path, "a.deb")
    write_asset(tmp_path, "extra.rpm")
    digest = release_digest_files.digest_of_file(asset)
    with pytest.raises(release_payload_tree.PayloadHandoffError, match="adds"):
        release_payload_tree.require_payload_matches(tmp_path, {"a.deb": digest})


def test_require_payload_matches_refuses_replaced_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a payload whose bytes are not the recorded ones."""
    write_asset(tmp_path, "a.deb", b"tampered")
    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        release_payload_tree.require_payload_matches(tmp_path, {"a.deb": "b" * 64})


def test_a_producer_record_is_not_a_payload_asset(tmp_path: pathlib.Path) -> None:
    """Keep the hand-off check from accepting a record as an asset."""
    write_provenance(tmp_path, "linux-x86_64")
    with pytest.raises(release_payload_tree.PayloadHandoffError, match="omits"):
        release_payload_tree.require_payload_matches(tmp_path, {"a.deb": "b" * 64})
