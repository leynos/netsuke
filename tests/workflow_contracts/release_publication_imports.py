"""Check network and external-effect boundaries in release modules.

Only the registry transport CLI and its HTTP adapter may import a network client;
policy modules additionally avoid filesystem access, serialization, HTTP
response vocabulary, and provider formats. Import spellings are easy to get
subtly wrong:
``import urllib.request``, ``from urllib import request``, and
``import urllib.request as fetch`` all reach the same client, while
``import release_digest`` reaches nothing.

Kept apart from the contracts that assert with it so each file stays under the
repository's 400-line cap, following :mod:`scripts.hoist_binstall_archives`
and its sibling discovery module.

Run via ``make test-workflow-contracts``.
"""

import ast
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Modules no release source may import, because each one opens a socket, a
#: URL, or a process. A bare name also bans its submodules: ``urllib`` covers
#: ``urllib.request``, which is where the client lives.
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

#: Imports that belong in workflow adapters and CLIs, not release policy.
POLICY_EFFECT_MODULES = frozenset({
    "hashlib",
    "http",
    "json",
    "os",
    "tomllib",
    "urllib",
})
PURE_PATH_CONSTRUCTORS = frozenset({"PurePath", "PurePosixPath", "PureWindowsPath"})
POLICY_EFFECT_CALLS = frozenset({
    "open",
    "mkdir",
    "stat",
    "is_file",
    "glob",
    "rglob",
    "unlink",
    "replace",
    "read_text",
    "read_bytes",
    "write_text",
    "write_bytes",
    "iterdir",
    "scandir",
    "walk_files",
    "loads",
    "dumps",
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


_HTTP_STATUS_FIELDS = frozenset({"status", "http_status", "status_code"})
_HTTP_STATUSES = frozenset({200, 403, 404})


def _import_offenders(node: ast.AST) -> set[str]:
    """Return adapter-owned imports found on one syntax-tree node.

    Returns
    -------
    set[str]
        The import descriptions owned by effectful adapters.

    Examples
    --------
    >>> _import_offenders(ast.parse("import json").body[0])
    {'import json'}
    """
    match node:
        case ast.Import(names=names):
            return {
                f"import {alias.name}"
                for alias in names
                if alias.name.split(".", maxsplit=1)[0] in POLICY_EFFECT_MODULES
                or alias.name == "pathlib"
            }
        case ast.ImportFrom(module=str() as module, names=names):
            if module.split(".", maxsplit=1)[0] in POLICY_EFFECT_MODULES:
                return {f"import {module}"}
            if module == "pathlib" and any(
                name.name not in PURE_PATH_CONSTRUCTORS for name in names
            ):
                return {"import pathlib.Path"}
    return set()


def _status_field_offender(node: ast.AST) -> set[str]:
    """Return a finding when a syntax node reads an HTTP status field.

    Returns
    -------
    set[str]
        The status-field finding, or an empty set.

    Examples
    --------
    >>> _status_field_offender(ast.parse("status").body[0].value)
    {'HTTP status field'}
    """
    match node:
        case ast.Name(id=field) | ast.Attribute(attr=field) | ast.arg(arg=field) if (
            field in _HTTP_STATUS_FIELDS
        ):
            return {"HTTP status field"}
    return set()


def _constant_offender(node: ast.AST) -> set[str]:
    """Return provider-specific status or digest literals on one node.

    Returns
    -------
    set[str]
        Descriptions of status or digest literals that belong at an adapter.

    Examples
    --------
    >>> _constant_offender(ast.parse("status == 404").body[0].value.comparators[0])
    {'HTTP status code'}
    """
    match node:
        case ast.Constant(value=int() as value) if value in _HTTP_STATUSES:
            return {"HTTP status code"}
        case ast.Constant(value=str() as value) if value.startswith("sha256:"):
            return {"GitHub digest prefix"}
    return set()


def _call_offender(node: ast.AST) -> set[str]:
    """Return file or serialization calls that belong at an adapter.

    Returns
    -------
    set[str]
        Descriptions of adapter-owned calls made by the syntax node.

    Examples
    --------
    >>> _call_offender(ast.parse("Path('x').read_text()").body[0].value)
    {'boundary call read_text'}
    """
    match node:
        case ast.Call(func=ast.Attribute(attr=str() as attribute)):
            if attribute in POLICY_EFFECT_CALLS:
                return {f"boundary call {attribute}"}
        case ast.Call(func=ast.Name(id=str() as name)):
            if name in POLICY_EFFECT_CALLS:
                return {f"boundary call {name}"}
    return set()


def _node_offenders(node: ast.AST) -> set[str]:
    """Collect every boundary finding represented by one AST node.

    Returns
    -------
    set[str]
        Boundary findings associated with the node.

    Examples
    --------
    >>> _node_offenders(ast.parse("status").body[0].value)
    {'HTTP status field'}
    """
    return (
        _import_offenders(node)
        | _status_field_offender(node)
        | _constant_offender(node)
        | _call_offender(node)
    )


def policy_boundary_offenders(source: str) -> list[str]:
    r"""Return JSON, file, status, or vendor-format operations in policy source.

    Returns
    -------
    list[str]
        Sorted, distinct descriptions of operations owned by adapters.

    Examples
    --------
    >>> policy_boundary_offenders("import json\nPath('x').read_text()")
    ['boundary call read_text', 'import json']
    >>> policy_boundary_offenders("return observation.sha256")
    []
    """
    return sorted(
        offender
        for node in ast.walk(ast.parse(source))
        for offender in _node_offenders(node)
    )


__all__ = [
    "NETWORK_MODULES",
    "any_offender",
    "imported_modules",
    "network_offenders",
    "opens_a_socket",
    "policy_boundary_offenders",
]
