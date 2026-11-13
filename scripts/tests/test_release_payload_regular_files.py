"""Test the regular-file proof that bounds every payload copy.

``require_regular`` decides whether a staged path is a real file inside the
downloaded tree. The hostile shapes below are the ones a real download can
hold: a link that redirects a copy, a FIFO that would block it, a path that
walks out of the tree with ``..``, and a tree the operating system will not
describe. Each is refused for its own reason, so the cases are named for the
shape rather than parametrized together.

Run via ``make test-release-publication``.
"""

import os
import pathlib

import pytest
import release_payload_tree
from release_payload_errors import StagedFileMissingError, StagedFileUnreadableError
from release_payload_install_support import write_asset


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
    download never staged. What has to be refused is the *escape*: the part
    below the root steps above its own origin. The companion case
    :func:`test_require_regular_accepts_a_dotdot_that_stays_inside_the_root`
    is the other half of that distinction.
    """
    dist = tmp_path / "dist"
    (dist / "sub").mkdir(parents=True)
    asset = write_asset(tmp_path / "outside", "a.deb")
    escaping = dist / "sub" / ".." / ".." / "outside" / asset.name
    assert escaping.is_relative_to(dist), "the lexical check must be the weak one"
    with pytest.raises(StagedFileMissingError, match="inside the staged tree"):
        release_payload_tree.require_regular(escaping, staged_root=dist)


def test_require_regular_accepts_a_dotdot_that_stays_inside_the_root(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a ``..`` that cancels back under the root, not above it.

    ``dist/sub/../sub/a.deb`` writes a ``..`` and resolves to a file producers
    really staged. Refusing it on the spelling alone would reject a good
    payload for the caller's phrasing, which is the false refusal a blanket
    ``..`` ban invites.
    """
    dist = tmp_path / "dist"
    asset = write_asset(dist / "sub", "a.deb")
    dotted = dist / "sub" / ".." / "sub" / asset.name
    assert release_payload_tree.require_regular(dotted, staged_root=dist) == dotted, (
        "a ``..`` that stays under the root names a staged file"
    )


def test_require_regular_accepts_a_staged_root_spelled_with_dotdot(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a good file whose *root* is spelled with a ``..``.

    ``os.walk`` does not resolve, and ``walk_files`` builds each result as
    ``Path(directory) / name``, so a root of ``dist/sub/..`` yields paths that
    carry that ``..`` into every source. The ``..`` cancels inside the root and
    the file is genuinely staged, so refusing it would fail a download for the
    caller's spelling -- the false refusal a blanket ``..`` ban produces.
    """
    dist = tmp_path / "dist"
    asset = write_asset(dist / "sub", "a.deb")
    dotted_root = dist / "sub" / ".."
    walked = dotted_root / "sub" / asset.name
    assert (
        release_payload_tree.require_regular(walked, staged_root=dotted_root) == walked
    ), "only the part below the root decides containment"


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
