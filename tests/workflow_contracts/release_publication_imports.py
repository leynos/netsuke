"""Decide whether a module can reach the network through what it imports.

The release-policy modules decide *what* a published byte is. Only workflow
steps touch the network, so that the decision itself stays testable without
one. That boundary is a property of the import list, and reading it is easier
to get subtly wrong than it looks: ``import urllib.request``, ``from urllib
import request``, and ``import urllib.request as fetch`` all reach the same
client, while ``import release_digest`` reaches nothing.

Kept apart from the contracts that assert with it so each file stays under the
repository's 400-line cap, following :mod:`scripts.hoist_binstall_archives`
and its sibling discovery module.

Run via ``make test-workflow-contracts``.
"""

import ast
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Modules a release-policy module must never import, because each one opens a
#: socket, a URL, or a process that does. A bare name also bans its submodules:
#: ``urllib`` covers ``urllib.request``, which is where the client lives.
NETWORK_MODULES = frozenset({
    "ftplib",
    "http",
    "http.client",
    "httpx",
    "requests",
    "socket",
    "ssl",
    "urllib",
    "urllib.request",
    "urllib3",
})


def imported_modules(source: str) -> set[str]:
    """Return every module name a source file imports, however it is spelled.

    Parameters
    ----------
    source
        Python source text.

    Returns
    -------
    set[str]
        Every imported module name, with ``import a.b`` recorded as ``a.b``.

    Notes
    -----
    Covers ``import x``, ``import x.y as z``, ``from x import y``, and
    ``from x.y import z``. Relative imports yield no name and are ignored:
    they name a sibling, never a standard-library client.

    Examples
    --------
    >>> sorted(imported_modules("import urllib.request"))
    ['urllib.request']
    >>> sorted(imported_modules("from pathlib import Path"))
    ['pathlib']
    """
    imported: set[str] = set()
    for node in ast.walk(ast.parse(source)):
        match node:
            case ast.Import():
                imported.update(alias.name for alias in node.names)
            case ast.ImportFrom(module=str() as module):
                imported.add(module)
    return imported


def opens_a_socket(module: str) -> bool:
    """Report whether an imported module name is a network client or a parent of one.

    Parameters
    ----------
    module
        A module name, as :func:`imported_modules` records it.

    Returns
    -------
    bool
        ``True`` when the name is, or is nested under, a banned module.

    Examples
    --------
    >>> opens_a_socket("urllib.request")
    True
    >>> opens_a_socket("release_digest")
    False
    """
    return any(
        module == banned or module.startswith(f"{banned}.")
        for banned in NETWORK_MODULES
    )


def network_offenders(source: str) -> list[str]:
    """Return the banned imports in ``source``, sorted, for a readable report.

    Parameters
    ----------
    source
        Python source text.

    Returns
    -------
    list[str]
        The offending module names, deduplicated and ordered.

    Examples
    --------
    >>> network_offenders("from socket import create_connection")
    ['socket']
    >>> network_offenders("from pathlib import Path")
    []
    """
    return sorted(name for name in imported_modules(source) if opens_a_socket(name))


def any_offender(names: cabc.Iterable[str]) -> bool:
    """Report whether any name in ``names`` reaches a network client.

    Parameters
    ----------
    names
        Module names, as :func:`imported_modules` records them.

    Returns
    -------
    bool
        ``True`` when at least one name is banned.
    """
    return any(opens_a_socket(name) for name in names)


__all__ = [
    "NETWORK_MODULES",
    "any_offender",
    "imported_modules",
    "network_offenders",
    "opens_a_socket",
]
