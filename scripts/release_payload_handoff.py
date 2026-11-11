"""Hold the installed payload to the bytes the trusted record declares.

Selection copies the approved sources into a fresh directory and writes the
record that describes them. That directory is then handed to another step, and
the stretch in between is one selection cannot police: the action copies the
payload under ``runner.temp`` and passes that local path to the uploader, so an
appended file, a replaced entry, or a step that rewrote a name is not something
selection can prevent.

This module is that second claim. It asks two questions of the directory it is
handed -- does it hold exactly the declared set of names, and are the bytes
under each name the ones the record demands -- and it asks them of the *same
paths* :func:`release_upload_remote.upload` will pass to ``gh``.

That last part is what makes the check mean anything. Indexing the directory by
published name answers a question about names, not about the file that is
actually sent: a regular ``sub/a.deb`` would satisfy the digest check for a
declared ``a.deb`` while a symlink at the root carried the transfer somewhere
else entirely. So each declared name is resolved to ``payload_dir / name`` and
proven a regular file there, by ``lstat``, before its bytes are read.

Run via ``scripts/upload_declared_assets_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_payload_handoff.py``.
"""

import stat
import typing as typ

import release_digest_files
import release_digest_record
import release_payload_regular_files
import release_payload_tree
from release_payload_errors import PayloadError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

__all__ = [
    "PayloadHandoffError",
    "require_payload_matches",
]


class PayloadHandoffError(PayloadError):
    """Report a payload directory that is not the set the record declares.

    The selection stage is the only writer of this directory, so a mismatch
    means the hand-off -- not the release -- is wrong, and uploading any part
    of it would publish bytes nobody selected.
    """


class _HandoffDisagreementError(PayloadHandoffError):
    """Report exactly which declared names the payload omits and adds."""

    def __init__(self, missing: list[str], extra: list[str]) -> None:
        """Name both sides of the disagreement in one operator-readable line."""
        super().__init__(f"payload omits {missing!r} and adds {extra!r}")


class _HandoffLeafError(PayloadHandoffError):
    """Report a declared name the uploader cannot hand to ``gh`` as a file.

    Deliberately a hand-off disagreement rather than a staging refusal. The
    named path is inside the payload the selection stage installed, so the
    hand-off is what is wrong -- the same class of fault as a payload that lost
    or gained a name -- and an operator fixes it by re-running the hand-off,
    not by changing what the producers stage.
    """

    def __init__(self, name: str, path: pathlib.Path) -> None:
        """Name the declared asset and the entry that cannot carry it."""
        super().__init__(
            f"payload cannot upload {name!r}: {path} is not a regular file "
            f"directly in {path.parent}"
        )


def _upload_leaf(payload_dir: pathlib.Path, name: str) -> pathlib.Path:
    """Return the exact path the uploader hands to ``gh`` for ``name``.

    Returns
    -------
    pathlib.Path
        ``payload_dir / name``, once proven a regular file.

    Raises
    ------
    _HandoffLeafError
        If that path is not a regular file. ``lstat`` decides, so a symlink is
        refused as itself rather than followed to whatever it names.

    Notes
    -----
    :func:`release_upload_remote.upload` sends ``payload_dir / name``; this is
    that same join, deliberately, because the check and the transfer must
    judge one path rather than two that are only usually the same. Hashing
    whichever file the walk found first under the published name would let a
    regular ``sub/name`` vouch for a root-level symlink: the walk skips the
    link, the nested file carries the trusted digest, and ``gh`` receives the
    link's target.

    A declared name that nests is not refused here on its own account -- the
    uploader would send it, and it is proven regular either way. Selection has
    already refused a name that escapes its directory.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     _ = (root / "real.deb").write_bytes(b"deb")
    ...     _ = (root / "a.deb").symlink_to(root / "real.deb")
    ...     (root / "sub").mkdir()
    ...     _ = (root / "sub" / "a.deb").write_bytes(b"deb")
    ...     try:
    ...         _upload_leaf(root, "a.deb")
    ...     except PayloadHandoffError as refusal:
    ...         print("cannot upload" in str(refusal))
    True
    """
    leaf = payload_dir / name
    if not release_payload_regular_files.has_mode(leaf, stat.S_ISREG):
        raise _HandoffLeafError(name, leaf)
    return leaf


def require_payload_matches(
    payload_dir: pathlib.Path, expected: cabc.Mapping[str, str]
) -> None:
    """Require the payload directory to hold exactly the declared bytes.

    Parameters
    ----------
    payload_dir
        The directory the selection stage installed.
    expected
        Published name mapped to the digest the trusted record demands.

    Raises
    ------
    _HandoffDisagreementError
        If the directory names any other set of files.

    Notes
    -----
    A declared name that is not a regular file directly in ``payload_dir`` --
    the path the uploader would send -- surfaces ``_HandoffLeafError`` from
    :func:`_upload_leaf`.

    A stored file whose bytes differ surfaces
    ``release_digest_record.RecordedDigestMismatchError`` from
    :func:`release_digest_record.require_downloads_match_record`.

    Selection verified these bytes before copying them; this checks the
    *hand-off*, which is a different claim. A job that restored the wrong
    artefact, or a step that appended a file, is caught here rather than after
    the upload.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     _ = (root / "a.deb").write_bytes(b"deb")
    ...     digest = release_digest_files.digest_of_file(root / "a.deb")
    ...     require_payload_matches(root, {"a.deb": digest})
    ...     try:
    ...         require_payload_matches(root, {})
    ...     except PayloadHandoffError as refusal:
    ...         print("adds" in str(refusal))
    True
    """
    present = release_payload_tree.observe_payload(payload_dir)
    missing = sorted(set(expected) - set(present))
    # A name two files publish under is refused alongside an undeclared name.
    # Without this, the walk's first match decides which copy is hashed: an
    # appended `sub/a.deb` beside `a.deb` leaves `missing` and `extra` both
    # empty, and the file this check exists to catch is never read at all.
    duplicated = {name for name, paths in present.items() if len(paths) > 1}
    extra = sorted((set(present) - set(expected)) | duplicated)
    if missing or extra:
        raise _HandoffDisagreementError(missing, extra)
    computed = {
        name: release_digest_files.digest_of_file(_upload_leaf(payload_dir, name))
        for name in sorted(expected)
    }
    release_digest_record.require_downloads_match_record(
        computed,
        trusted=release_digest_record.DigestRecord.from_mapping(expected),
    )
