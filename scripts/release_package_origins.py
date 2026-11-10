"""Observe which producer's downloaded subtree staged each release package.

A package name cannot be derived from configuration: three external shared
actions own the ``.deb``, ``.rpm``, ``.pkg`` and ``.msi`` names, so the suffix
and the per-target count are all the name itself says. Two same-platform
targets that trade one package each therefore leave every count unchanged and
the release's union of names identical, and a count-based check accepts the
trade.

Position supplies what the name cannot. Every producer uploads exactly one
workflow artefact, the release downloads each into its own top-level directory
below ``dist``, and the producer writes its record inside the tree it uploads.
A record's own top-level directory is consequently the artefact that producer
uploaded, and every package found below that directory was staged by the
target the record names.

That is an identity proof rather than a count, and it is available before any
upload: the observation is taken from the same downloaded tree selection
already walks.

Only names are read here -- no file is opened and no record is parsed. The
record is located by its name, read by :mod:`release_payload_tree`, and its
``target`` is what attributes the subtree.

Run via ``scripts/select_release_payload_cli.py``; behavioural coverage lives
in ``scripts/tests/test_release_package_origins.py``.
"""

import typing as typ

import hoist_binstall_discovery
import release_github_assets_adapter
import release_payload_names

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


__all__ = ["observe_package_origins"]


def observe_package_origins(
    dist_dir: pathlib.Path,
    *,
    target_of_record: cabc.Mapping[pathlib.Path, str],
) -> dict[str, str]:
    """Map each staged package name to the target whose subtree holds it.

    Parameters
    ----------
    dist_dir
        Release staging root the workflow artefacts were downloaded into.
    target_of_record
        Producer record path mapped to the target that record names, as
        :func:`release_payload_tree.read_provenances` observed it.

    Returns
    -------
    dict[str, str]
        Published package name mapped to the target key of the subtree it was
        staged in. A name staged in two subtrees is absent from this mapping's
        single value, because the staged-tree check refuses an ambiguous name
        on its own and a guess here would blame the wrong producer.

    Notes
    -----
    A tree that cannot be traversed propagates ``OSError`` from
    :func:`hoist_binstall_discovery.walk_files`, the same traversal selection
    already relies on, so an unreadable directory cannot masquerade as a tree
    with no packages.

    A package staged outside any identified subtree -- below the ``dist`` root
    itself, where the hoist leaves archives -- is not attributed. Archives are
    bound by *name* instead, and those are all that legitimately sit there.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     observe_package_origins(Path(tmp), target_of_record={})
    {}
    """
    seen: dict[str, str] = {}
    ambiguous: set[str] = set()
    for record_path, target in sorted(target_of_record.items()):
        subtree = _first_segment_below(dist_dir, record_path)
        if subtree is None:
            continue
        _attribute(seen, ambiguous, _packages_below(dist_dir / subtree), target)
    return {name: target for name, target in seen.items() if name not in ambiguous}


def _attribute(
    seen: dict[str, str],
    ambiguous: set[str],
    names: cabc.Iterable[str],
    target: str,
) -> None:
    """Attribute one producer's package names to the target it names.

    Parameters
    ----------
    seen
        Package name mapped to the target whose subtree first staged it.
    ambiguous
        Names already observed under two different targets.
    names
        The package names staged below this producer's subtree.
    target
        The target this subtree's record names.

    Notes
    -----
    A name already marked ambiguous is marked again rather than skipped, so a
    third subtree that agrees with the first does not unlearn the second one's
    disagreement. Both collections are updated together here because deciding
    a clash and recording it are one fact; split across the caller, a reader
    had to hold two loops in mind to see that no name could be both.
    """
    for name in names:
        if seen.setdefault(name, target) != target:
            ambiguous.add(name)


def _first_segment_below(root: pathlib.Path, path: pathlib.Path) -> str | None:
    """Return the top-level directory of ``root`` that contains ``path``.

    Returns
    -------
    str | None
        The single path segment naming the downloaded artefact, or ``None``
        when ``path`` sits directly in ``root`` and so names no artefact.

    Examples
    --------
    >>> from pathlib import Path
    >>> _first_segment_below(Path("dist"), Path("dist/art/b/record.json"))
    'art'
    >>> _first_segment_below(Path("dist"), Path("dist/record.json")) is None
    True
    """
    try:
        relative = path.relative_to(root)
    except ValueError:
        return None
    return relative.parts[0] if len(relative.parts) > 1 else None


def _packages_below(subtree: pathlib.Path) -> list[str]:
    """Return the package names staged anywhere below ``subtree``.

    Returns
    -------
    list[str]
        Each package's published name, in walk order.

    Notes
    -----
    Only names carrying a package suffix are returned. Archives and their
    sidecars are hoisted to the ``dist`` root and bound by name, so a copy
    left in a producer's subtree is not evidence of that producer's target.
    """
    return [
        release_github_assets_adapter.published_asset_name(path.name)
        for path in hoist_binstall_discovery.walk_files(subtree)
        if release_payload_names.is_package(path.name)
    ]
