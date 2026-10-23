r"""Judge whether a staged asset name is safe to publish as a bare file name.

An asset name is data, not a path on the host that produced it, so it is
judged under both path flavours. ``C:\Windows\netsuke.exe`` is one ordinary
component to a POSIX parser and something else entirely to an uploader running
on Windows; a name absolute under *either* reading is refused.

Two defects are distinguished here, and the distinction matters. A name that
walks above its origin (``../netsuke``) means the release tried to publish
outside its staging directory -- a security diagnosis. A name that is merely
nested or reserved (``sub/netsuke``) means a directory-shaped path reached a
listing that should hold flat files. Reporting both as "bad name" would hide
the first.

Nothing here raises or performs I/O: :mod:`release_inventory` owns the
refusals, so the predicates can be exercised directly.

Run via ``scripts/release_inventory.py``; behavioural coverage lives in
``scripts/tests/test_release_inventory.py``.
"""

import typing as typ
from pathlib import PurePosixPath, PureWindowsPath

if typ.TYPE_CHECKING:
    import collections.abc as cabc

PATH_FLAVOURS = (PurePosixPath, PureWindowsPath)


def walks_out_of_its_directory(parts: tuple[str, ...]) -> bool:
    """Report whether walking ``parts`` in order ever goes above the start.

    Parameters
    ----------
    parts
        Path components, already split by whichever flavour produced them.

    Returns
    -------
    bool
        ``True`` when a ``..`` component is reached with no preceding
        component left to cancel it, so the path resolves above its origin.

    Notes
    -----
    Counting depth is what catches ``a/../../b``: the leading ``a`` looks
    harmless and the traversal happens two components later.

    Examples
    --------
    >>> walks_out_of_its_directory(("a", "..", "b"))
    False
    >>> walks_out_of_its_directory(("a", "..", "..", "b"))
    True
    """
    depth = 0
    for part in parts:
        if part == "..":
            depth -= 1
            if depth < 0:
                return True
        elif part not in {"", "."}:
            depth += 1
    return False


def escapes_its_directory(name: str) -> bool:
    r"""Report whether ``name`` could resolve outside the directory holding it.

    Parameters
    ----------
    name
        Candidate asset name to judge.

    Returns
    -------
    bool
        ``True`` for an absolute path, a Windows drive or UNC path, or any
        component sequence that walks above its origin.

    Notes
    -----
    Both path flavours are consulted for the reason the module docstring
    gives: a name absolute under *either* reading is refused.
    """
    for flavour in PATH_FLAVOURS:
        parsed = flavour(name)
        if parsed.is_absolute() or walks_out_of_its_directory(parsed.parts):
            return True
    return False


def is_bare_file_name(name: str) -> bool:
    """Report whether ``name`` is a single, non-empty, flat component.

    Parameters
    ----------
    name
        Candidate asset name to judge.

    Returns
    -------
    bool
        ``True`` when the name is exactly one component under both path
        flavours, with no empty component and no reserved spelling such as
        ``.`` or ``..``.

    Notes
    -----
    Both flavours must agree, for the reason given in
    :func:`escapes_its_directory`: a name nested under Windows separators
    alone is not the flat name the uploader publishes, though a POSIX parser
    treats the backslashes as ordinary characters.
    """
    return all(
        len(flavour(name).parts) == 1 and flavour(name).parts[0] not in {"", ".", ".."}
        for flavour in PATH_FLAVOURS
    )


def rejected_names(names: cabc.Iterable[str]) -> tuple[list[str], list[str]]:
    """Partition ``names`` into those escaping and those merely malformed.

    Parameters
    ----------
    names
        Candidate asset names, in the order to report them.

    Returns
    -------
    tuple[list[str], list[str]]
        The escaping names and the malformed names, each in input order.

    Notes
    -----
    The two are different diagnoses, as the module docstring explains, and the
    caller reports them as separate refusals.

    Examples
    --------
    >>> rejected_names(["a.tar.gz", "../x", "sub/y"])
    (['../x'], ['sub/y'])
    """
    escaping: list[str] = []
    malformed: list[str] = []
    for name in names:
        if escapes_its_directory(name):
            escaping.append(name)
        elif not is_bare_file_name(name):
            malformed.append(name)
    return escaping, malformed
