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

The walk stops at the staged root the caller names. That root and everything
below it is the downloaded tree, so a link anywhere inside it redirects a
copy; its own ancestors are the runner's path, chosen by the job rather than by
the producers, and a link there redirects nothing this run publishes. Probing
them anyway would reject a perfectly good tree on a host whose
``$RUNNER_TEMP`` sits under a symlinked mount -- a false refusal with no
corrective action an operator could take.

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

__all__ = ["copy_verified", "has_mode", "require_regular"]


def copy_verified(
    source: pathlib.Path,
    destination: pathlib.Path,
    *,
    staged_root: pathlib.Path,
) -> None:
    """Copy ``source`` after proving it and its staged ancestors are regular.

    Parameters
    ----------
    source
        The staged file to copy.
    destination
        Where to copy it to.
    staged_root
        The downloaded tree ``source`` was staged inside. Directories from it
        down to the source are proven; anything above it is the runner's own
        path and is not.

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
    ...     copy_verified(source, root / "out.deb", staged_root=root)
    ...     (root / "out.deb").read_bytes()
    b'deb'
    """
    require_regular(source, staged_root=staged_root)
    _ = shutil.copyfile(source, destination)


def require_regular(path: pathlib.Path, *, staged_root: pathlib.Path) -> pathlib.Path:
    """Return ``path`` once it and its staged ancestors are proven regular.

    Parameters
    ----------
    path
        The staged file to prove.
    staged_root
        The downloaded tree ``path`` was staged inside. Every directory from
        this one down to ``path`` must be a real directory.

    Returns
    -------
    pathlib.Path
        The same path, once proven regular.

    Raises
    ------
    StagedFileMissingError
        If the leaf is absent or not a regular file, if an ancestor between
        ``staged_root`` and the leaf is not a directory, or if ``path`` is not
        below ``staged_root`` at all.

    Notes
    -----
    ``StagedFileUnreadableError`` reaches the caller from :func:`has_mode`
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

    The walk starts at ``staged_root`` rather than running to ``/``. A path
    outside the root is refused as well, because a source selection did not
    observe is a different claim from one it did, and the containment is what
    makes "the downloaded tree" a statement about a directory rather than about
    however the caller happened to spell a path.

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
    ...         require_regular(root / "link.deb", staged_root=root)
    ...     except StagedFileMissingError as refusal:
    ...         print("regular file" in str(refusal))
    True
    """
    _require_below(path, staged_root)
    if not has_mode(path, stat.S_ISREG):
        raise StagedFileMissingError(path, expected="a regular file")
    for ancestor in path.parents:
        # The root is proven and the walk stops there: `parents` alone runs to
        # `/`, and a link above the tree the producers staged says nothing
        # about the bytes this run publishes.
        if not ancestor.is_relative_to(staged_root):
            break
        if not has_mode(ancestor, stat.S_ISDIR):
            raise StagedFileMissingError(ancestor, expected="a directory")
    return path


def _require_below(path: pathlib.Path, staged_root: pathlib.Path) -> None:
    """Require ``path`` to name an entry below ``staged_root``.

    Raises
    ------
    StagedFileMissingError
        If ``path`` lies outside ``staged_root``.

    Notes
    -----
    The comparison is lexical, as it is everywhere in this module: ``dist/a``
    and ``./dist/a`` name the same entry and are accepted, while a sibling
    subtree or a path elsewhere is not. Resolving either side would follow
    exactly the links :func:`require_regular` exists to judge.

    This is also what keeps the bounded walk bounded. Without it, a path
    outside the root would exhaust ``parents`` and reach ``/`` again, which is
    the unbounded probe the root is passed in to avoid.

    Examples
    --------
    >>> from pathlib import Path
    >>> _require_below(Path("dist/a.deb"), Path("dist"))
    >>> _require_below(Path("dist"), Path("dist/"))
    >>> _require_below(Path("elsewhere"), Path("dist"))  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_payload_errors.StagedFileMissingError: elsewhere is not inside ...
    """
    if path.is_relative_to(staged_root):
        return
    raise StagedFileMissingError(
        path, expected=f"inside the staged tree {staged_root!r}"
    )


def has_mode(path: pathlib.Path, predicate: cabc.Callable[[int], bool]) -> bool:
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
    >>> import pathlib
    >>> import tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     _ = (pathlib.Path(tmp) / "a").write_bytes(b"x")
    ...     has_mode(pathlib.Path(tmp) / "a", stat.S_ISREG)
    True
    >>> has_mode(pathlib.Path("/nonexistent/a"), stat.S_ISREG)
    False
    """
    try:
        return predicate(path.lstat().st_mode)
    except FileNotFoundError, NotADirectoryError:
        return False
    except OSError as error:
        raise StagedFileUnreadableError(path, error) from error
