"""Prove a staged path regular, then copy it, judging links as themselves.

A payload copy is trustworthy only if every byte it carries came from a real
file inside the downloaded tree. A symlinked leaf or a symlinked producer
directory would satisfy every name-based check and still hand the release
whatever the link points at, so this module asks the filesystem what each
entry *is* rather than what it is called.

``lstat`` is used throughout for that reason: following a link would answer a
question about the target instead of about the staged tree. Absence and
unreadability are answered differently on purpose. A path the operating system
will not describe is not evidence the file is missing, and calling it absent
would send an operator to fix a staging step that did the right thing.

Run via :mod:`release_payload_tree`; behavioural coverage lives in
``scripts/tests/test_release_payload_tree.py``.
"""

import shutil
import stat
import typing as typ

from release_payload_errors import StagedFileMissingError, StagedFileUnreadableError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

__all__ = ["copy_verified", "require_regular"]


def copy_verified(source: pathlib.Path, destination: pathlib.Path) -> None:
    """Copy ``source`` after proving it and its ancestors are regular files.

    Notes
    -----
    A path that is not a regular file below regular directories surfaces
    :class:`release_payload_errors.StagedFileMissingError` from
    :func:`require_regular`, and a copy failure propagates ``OSError``.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     source = root / "asset.deb"
    ...     _ = source.write_bytes(b"deb")
    ...     copy_verified(source, root / "out.deb")
    ...     (root / "out.deb").read_bytes()
    b'deb'
    """
    require_regular(source)
    _ = shutil.copyfile(source, destination)


def require_regular(path: pathlib.Path) -> pathlib.Path:
    """Return ``path`` when it and every ancestor below the root are regular.

    Returns
    -------
    pathlib.Path
        The same path, once proven regular.

    Raises
    ------
    StagedFileMissingError
        If the leaf is absent or not a regular file, or any ancestor is not a
        directory.

    Notes
    -----
    ``StagedFileUnreadableError`` reaches the caller from :func:`_has_mode`
    when ``lstat`` refuses to describe the leaf or an ancestor, so it is
    described here rather than in ``Raises``.

    ``lstat`` is used throughout so a link is judged as itself: following a
    symlinked producer directory would let the payload hold bytes from outside
    the downloaded tree, which is the substitution the trusted record exists to
    catch. A vanished source is ``StagedFileMissingError`` rather than
    ``FileNotFoundError``, because from the caller's side a source that
    disappeared between observation and copy is the same refusal as one that
    was never regular, and the CLI maps the typed family to one exit code. A
    source the operating system will not describe keeps its own refusal: that
    is an environment fault, not a staging one.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     real = root / "real.deb"
    ...     _ = real.write_bytes(b"deb")
    ...     (root / "link.deb").symlink_to(real)
    ...     try:
    ...         require_regular(root / "link.deb")
    ...     except StagedFileMissingError as refusal:
    ...         print("regular file" in str(refusal))
    True
    """
    if not _has_mode(path, stat.S_ISREG):
        raise StagedFileMissingError(path, expected="a regular file")
    for ancestor in path.parents:
        if not _has_mode(ancestor, stat.S_ISDIR):
            raise StagedFileMissingError(ancestor, expected="a directory")
    return path


def _has_mode(path: pathlib.Path, predicate: cabc.Callable[[int], bool]) -> bool:
    """Report whether ``path``'s own mode satisfies ``predicate``.

    Returns
    -------
    bool
        ``True`` when the entry exists and its ``lstat`` mode satisfies
        ``predicate``; ``False`` when it is genuinely absent or of another
        type.

    Raises
    ------
    StagedFileUnreadableError
        If ``lstat`` fails for any reason other than absence, such as a
        permission refusal or an I/O fault.

    Notes
    -----
    Absence and unreadability are answered differently on purpose: a path the
    operating system will not describe is not evidence the file is not there,
    and calling it absent would send an operator to fix a staging step that did
    the right thing. Only ``ENOENT`` and ``ENOTDIR`` mean "no such entry"; the
    rest is an environment fault, and ``walk_files`` reaches this predicate
    too, so an unreadable entry stops selection rather than being skipped.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     _ = (Path(tmp) / "a").write_bytes(b"x")
    ...     _has_mode(Path(tmp) / "a", stat.S_ISREG)
    True
    >>> _has_mode(pathlib.Path("/nonexistent/a"), stat.S_ISREG)
    False
    """
    try:
        return predicate(path.lstat().st_mode)
    except FileNotFoundError, NotADirectoryError:
        return False
    except OSError as error:
        raise StagedFileUnreadableError(path, error) from error
