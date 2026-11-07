"""Check which release modules may launch a child process.

A child process is not a socket, but every source here is spawned from a
workflow step that already holds a credential, so a source that runs a command
is a place the release can reach an outside service. Two of them do and must:
``package_crate_cli`` runs ``cargo package``, and ``release_upload_remote``
runs ``gh release upload`` and ``gh release view``.

The distinction matters because the command, not the import, decides whether
the network is touched. ``subprocess`` alone says nothing about that, so a
blanket ban would misreport the packaging seam as a network offender and could
not be satisfied by the transport modules, whose contract requires a genuine
*network* import. Naming the spawners instead keeps the decision explicit:
adding one is a reviewed entry, not an omission nobody notices.

Import spellings are easy to get subtly wrong, so the same module-name rules
as the network boundary apply: ``import subprocess``, ``import subprocess as
sp``, and ``from subprocess import run`` all reach the same launcher.

Kept apart from :mod:`release_publication_imports` so each file stays under the
repository's 400-line cap, following the sibling-module pattern that module's
own docstring records.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Modules that launch a child process. A bare name also bans its submodules.
PROCESS_LAUNCHERS = frozenset({"subprocess"})


def _is_banned(module: str, banned_names: frozenset[str]) -> bool:
    """Report whether ``module`` is, or is nested under, one of ``banned_names``.

    Returns
    -------
    bool
        ``True`` when the name matches a banned module or its submodule.

    Examples
    --------
    >>> _is_banned("subprocess", PROCESS_LAUNCHERS)
    True
    >>> _is_banned("socket", PROCESS_LAUNCHERS)
    False
    """
    return any(
        module == banned or module.startswith(f"{banned}.") for banned in banned_names
    )


def starts_a_process(module: str) -> bool:
    """Report whether an imported module name can launch a child process.

    Parameters
    ----------
    module
        A module name, as :func:`release_publication_imports.imported_modules`
        records it.

    Returns
    -------
    bool
        ``True`` when the name is, or is nested under, a process launcher.

    Examples
    --------
    >>> starts_a_process("subprocess")
    True
    >>> starts_a_process("socket")
    False
    """
    return _is_banned(module, PROCESS_LAUNCHERS)


def process_offenders(source: str) -> list[str]:
    """Return the imports in ``source`` able to launch a process, sorted.

    Parameters
    ----------
    source
        Python source text.

    Returns
    -------
    list[str]
        The process-launching module names, deduplicated and ordered.

    Examples
    --------
    >>> process_offenders("import subprocess")
    ['subprocess']
    >>> process_offenders("from socket import create_connection")
    []
    """
    from release_publication_imports import imported_modules

    return sorted(name for name in imported_modules(source) if starts_a_process(name))


def any_process_offender(names: cabc.Iterable[str]) -> bool:
    """Report whether any name in ``names`` can launch a process.

    Parameters
    ----------
    names
        Module names, as :func:`release_publication_imports.imported_modules`
        records them.

    Returns
    -------
    bool
        ``True`` when at least one name can launch a process.
    """
    return any(starts_a_process(name) for name in names)


__all__ = [
    "PROCESS_LAUNCHERS",
    "any_process_offender",
    "process_offenders",
    "starts_a_process",
]
