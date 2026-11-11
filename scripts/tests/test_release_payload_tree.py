"""Test the filesystem boundary the payload selection reads and writes through.

The tree adapter is where a symlink stops being an ordinary file and where an
existing output directory stops being a merge target, so these cases build the
hostile shapes a real download can contain rather than asserting on names.

Run via ``make test-release-publication``.
"""

import json
import os
import pathlib

import pytest
import release_digest_files
import release_digest_record_files
import release_payload_selection
import release_payload_tree
from release_payload_errors import StagedFileMissingError, StagedFileUnreadableError
from release_payload_install_support import (
    selection_for,
    write_asset,
    write_provenance,
)
from release_payload_tree import RECORD_NAME


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
    assert release_payload_tree.require_regular(asset, staged_root=tmp_path) == asset, (
        "an ordinary file must be returned unchanged"
    )


def test_require_regular_ignores_a_link_above_the_staged_root(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a good tree whose own path runs through a link.

    A runner temp directory under a symlinked mount is the host's layout, not
    the producers' claim. Refusing there would reject a perfectly good tree
    with no corrective action available to an operator.
    """
    real = tmp_path / "mnt"
    dist = real / "dist"
    asset = write_asset(dist, "a.deb")
    linked = tmp_path / "runner-temp"
    _ = linked.symlink_to(real, target_is_directory=True)
    assert release_payload_tree.require_regular(
        linked / "dist" / asset.name, staged_root=linked / "dist"
    ) == (linked / "dist" / asset.name), "the staged root bounds the walk"


def test_require_regular_refuses_a_path_outside_the_staged_root(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a path the root does not contain, before any mode is read."""
    dist = tmp_path / "dist"
    dist.mkdir()
    asset = write_asset(tmp_path / "elsewhere", "a.deb")
    with pytest.raises(StagedFileMissingError, match="inside the staged tree"):
        release_payload_tree.require_regular(asset, staged_root=dist)


def test_require_regular_refuses_a_path_that_walks_out_with_dotdot(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a lexically relative path whose ``..`` reaches outside the root.

    ``is_relative_to`` compares components as written, so
    ``dist/sub/../../outside/a.deb`` answers ``True`` for ``dist``. Every
    ancestor down to ``dist`` is then a real directory, so a containment check
    that stopped at the comparison would pass this path and copy a file the
    download never staged. The ``..`` itself is what has to be refused.
    """
    dist = tmp_path / "dist"
    (dist / "sub").mkdir(parents=True)
    asset = write_asset(tmp_path / "outside", "a.deb")
    escaping = dist / "sub" / ".." / ".." / "outside" / asset.name
    assert escaping.is_relative_to(dist), "the lexical check must be the weak one"
    with pytest.raises(StagedFileMissingError, match="inside the staged tree"):
        release_payload_tree.require_regular(escaping, staged_root=dist)


def test_require_regular_refuses_a_symlinked_leaf(tmp_path: pathlib.Path) -> None:
    """Refuse a leaf that is a link, judged without following it."""
    real = write_asset(tmp_path / "real", "a.deb")
    link = tmp_path / "link.deb"
    _ = link.symlink_to(real)
    with pytest.raises(StagedFileMissingError, match="regular file"):
        release_payload_tree.require_regular(link, staged_root=tmp_path)


def test_require_regular_refuses_a_symlinked_ancestor(tmp_path: pathlib.Path) -> None:
    """Refuse a file reached through a linked directory inside the root.

    Following it would let the payload hold bytes from outside the downloaded
    tree, which is the substitution the trusted record exists to catch. The
    stage the link occupies is also what keeps this case distinct from
    ``test_require_regular_ignores_a_link_above_the_staged_root``: there the
    link is the runner's own path, here it is inside the tree.
    """
    real_dir = tmp_path / "elsewhere"
    asset = write_asset(real_dir, "a.deb")
    linked_dir = tmp_path / "producer"
    _ = linked_dir.symlink_to(real_dir, target_is_directory=True)
    with pytest.raises(StagedFileMissingError, match="directory"):
        release_payload_tree.require_regular(
            linked_dir / asset.name, staged_root=tmp_path
        )


def test_require_regular_refuses_a_fifo(tmp_path: pathlib.Path) -> None:
    """Refuse a non-regular leaf that is not a link.

    A FIFO in a downloaded tree would block a copy indefinitely, which a release
    job cannot distinguish from a slow transfer. The leaf is checked before the
    ancestors, so a directory cannot be reached through this path; the
    ancestor branch is covered by
    :func:`test_require_regular_refuses_a_symlinked_ancestor`.
    """
    fifo = tmp_path / "a-fifo"
    os.mkfifo(fifo)
    with pytest.raises(StagedFileMissingError, match="regular file"):
        release_payload_tree.require_regular(fifo, staged_root=tmp_path)


def test_require_regular_refuses_a_missing_file(tmp_path: pathlib.Path) -> None:
    """Surface the absent path rather than a bare ``FileNotFoundError``."""
    with pytest.raises(StagedFileMissingError, match="regular file"):
        release_payload_tree.require_regular(
            tmp_path / "absent.deb", staged_root=tmp_path
        )


def test_require_regular_reports_an_unreadable_leaf(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep a permission refusal distinct from an absent file.

    Absence and unreadability need different fixes: one is a staging bug, the
    other is a permission, mount, or I/O fault. Reporting the second as the
    first would send an operator to a step that did the right thing.

    ``lstat`` is patched rather than the mode changed, because a test may run
    as a user that ignores the permission bits entirely -- root certainly does
    -- and the refusal would then never be reached.
    """
    asset = write_asset(tmp_path, "a.deb")

    def refuse(_path: pathlib.Path) -> os.stat_result:
        """Refuse every ``lstat``, as an unreadable file would."""
        raise PermissionError(13, "Permission denied")

    monkeypatch.setattr(pathlib.Path, "lstat", refuse)
    with pytest.raises(StagedFileUnreadableError, match="could not be inspected"):
        release_payload_tree.require_regular(asset, staged_root=tmp_path)


def test_require_regular_reports_an_unreadable_ancestor(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Apply the same distinction to an ancestor probe.

    The leaf is read first and is genuine here, so only the ancestor loop can
    raise; that is what makes this the ancestor's case rather than a repeat of
    the leaf's.
    """
    asset = write_asset(tmp_path, "a.deb")
    genuine_lstat = pathlib.Path.lstat

    def refuse_ancestors(path: pathlib.Path) -> os.stat_result:
        """Refuse the ancestor probe while leaving the leaf readable."""
        if path == tmp_path:
            raise PermissionError(13, "Permission denied")
        return genuine_lstat(path)

    monkeypatch.setattr(pathlib.Path, "lstat", refuse_ancestors)
    with pytest.raises(StagedFileUnreadableError, match="could not be inspected"):
        release_payload_tree.require_regular(asset, staged_root=tmp_path)


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
