"""Judge a staged tree's names against the release declaration.

Selection decides *which* release is being published. This module answers the
narrower question the staged tree raises once that is settled: does every
declared name have exactly one file, and is every other release-shaped file
one the declaration accounts for?

The distinction between "extra" and "wrong" is the whole policy. Files that no
release publishes -- the binary, licences, completions, manual pages that ship
*inside* the packages -- are excluded silently, because refusing them would
reject a perfectly good build. A file that does carry a publishable suffix is
refused, because the only ways it can exist are a stale download, an ambiguous
name, or retained workspace material a careless uploader would publish.

Nothing here walks a tree, reads a file, or hashes bytes, so every refusal is
reachable from a plain dictionary of names.

Run via ``scripts/select_release_payload_cli.py``; behavioural coverage lives
in ``scripts/tests/test_release_payload_selection.py``.
"""

import typing as typ

import release_asset_names
import release_declaration
import release_package_formats

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "ARCHIVE_SUFFIX",
    "PACKAGE_SUFFIXES",
    "describe_offenders",
    "is_release_shaped",
]

#: Suffix of the cargo-binstall archive every target produces.
ARCHIVE_SUFFIX = ".tar.gz"

#: Every package suffix the configured platforms stage. Derived from the format
#: table rather than restated, so a platform added there cannot be forgotten.
PACKAGE_SUFFIXES = tuple(
    sorted({
        package_format
        for formats in release_package_formats.PLATFORM_FORMATS.values()
        for package_format in formats
    })
)


def is_release_shaped(name: str) -> bool:
    """Report whether a published name is one a release could ever publish.

    Parameters
    ----------
    name
        A published asset name.

    Returns
    -------
    bool
        ``True`` for an archive, an archive sidecar, or a package carrying one
        of the configured platform suffixes.

    Notes
    -----
    This decides what an *undeclared* file means. A build product such as
    ``netsuke``, ``LICENSE``, or a completion script is staged material that no
    release publishes, so it is excluded silently. A file carrying a
    publishable suffix is either this release's asset or something that would
    publish if it were uploaded, and selection refuses to guess which.

    Examples
    --------
    >>> is_release_shaped("netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz")
    True
    >>> is_release_shaped("netsuke-1.0.0-1.x86_64.rpm")
    True
    >>> is_release_shaped("netsuke-1.0.0-x86_64.tar.gz.sha256")
    True
    >>> is_release_shaped("completions/bash/netsuke")
    False
    """
    sidecar = f"{ARCHIVE_SUFFIX}{release_declaration.SIDECAR_SUFFIX}"
    return name.endswith((*PACKAGE_SUFFIXES, ARCHIVE_SUFFIX, sidecar))


def describe_offenders(
    declared: cabc.Iterable[str], staged: cabc.Mapping[str, cabc.Sequence[object]]
) -> list[str]:
    """Describe every way the staged tree disagrees with the declaration.

    Parameters
    ----------
    declared
        Asset names the release declares.
    staged
        Published name mapped to each observed file claiming it.

    Returns
    -------
    list[str]
        Human-readable offenders, sorted so a re-run reports the same list.

    Notes
    -----
    Unsafe names are reported before the files claiming them: a name that
    escapes its directory is a security diagnosis, and reporting it as a
    missing asset would hide it.

    Examples
    --------
    >>> describe_offenders(("a.deb",), {"a.deb": ["x"]})
    []
    >>> describe_offenders(("a.deb",), {})
    ["'a.deb' was not staged"]
    >>> describe_offenders(("a.deb",), {"a.deb": ["x", "y"]})
    ["'a.deb' was staged 2 times"]
    >>> describe_offenders(("a.deb",), {"../a.deb": ["x"]})[0]
    "staged name escapes its directory: '../a.deb'"
    """
    names = tuple(declared)
    escaping, malformed = release_asset_names.rejected_names(staged)
    unsafe = [
        *(f"staged name escapes its directory: {name!r}" for name in escaping),
        *(f"staged name is not a bare file name: {name!r}" for name in malformed),
    ]
    return [
        *sorted(unsafe),
        *sorted(_undescribed(names, staged, set(escaping) | set(malformed))),
    ]


def _undescribed(
    declared: tuple[str, ...],
    staged: cabc.Mapping[str, cabc.Sequence[object]],
    unsafe: set[str],
) -> list[str]:
    """Describe release-shaped strays and every defect in a declared name.

    Parameters
    ----------
    declared
        Asset names the release declares.
    staged
        Published name mapped to each observed file claiming it.
    unsafe
        Staged names already reported for their spelling.

    Returns
    -------
    list[str]
        Offenders for the names whose own spelling was safe.

    Examples
    --------
    >>> _undescribed(("b.rpm",), {"b.rpm": ["x"], "c.rpm": ["y"]}, set())
    ["'c.rpm' is release-shaped but not declared"]
    >>> _undescribed(("a.deb",), {"a.deb": ["x"]}, set())
    []
    """
    declared_set = set(declared)
    offenders = [
        f"{name!r} is release-shaped but not declared"
        for name in sorted(staged)
        if name not in declared_set and name not in unsafe and is_release_shaped(name)
    ]
    for name in declared:
        sources = staged.get(name, ())
        if not sources:
            offenders.append(f"{name!r} was not staged")
        elif len(sources) > 1:
            offenders.append(f"{name!r} was staged {len(sources)} times")
    return offenders
