"""Build the selections and staged trees the tree and install cases hand over.

The install half of :mod:`release_payload_tree` copies an approved selection
and writes the record beside it, and its cases vary one thing at a time: which
digests the selection carries, whether a source lies inside the staged root,
which source vanishes between the check and the copy. The staging around those
variations is identical, so it lives here rather than being rebuilt per case.

Only builders live in this module -- a helper that writes a file or assembles a
selection. The assertions stay in the cases that state them.

The helpers are public because the tree's own suite imports them as well: a
case that stages a tree and a case that installs one then read the same file
layout, and the tree suite stays below the module size the lint gate allows.
"""

import json
import typing as typ

import release_payload_selection
from release_payload_tree import RECORD_NAME

if typ.TYPE_CHECKING:
    import pathlib


def write_asset(
    directory: pathlib.Path, name: str, content: bytes = b"bytes"
) -> pathlib.Path:
    """Write ``content`` under ``name`` and return the path.

    Returns
    -------
    pathlib.Path
        The written file's path.
    """
    path = directory / name
    path.parent.mkdir(parents=True, exist_ok=True)
    _ = path.write_bytes(content)
    return path


def write_provenance(directory: pathlib.Path, target: str) -> None:
    """Write a valid producer record for ``target`` below ``directory``."""
    record = {
        "schema": 1,
        "commit": "a" * 40,
        "run_id": "7",
        "run_attempt": "1",
        "target": target,
        "platform": "linux",
        "assets": {"a.deb": "b" * 64},
    }
    directory.mkdir(parents=True, exist_ok=True)
    _ = (directory / RECORD_NAME).write_text(json.dumps(record), encoding="utf-8")


def selection_for(
    sources: dict[str, pathlib.Path],
) -> release_payload_selection.PayloadSelection[pathlib.Path]:
    """Name ``sources`` as the approved payload.

    Returns
    -------
    release_payload_selection.PayloadSelection
        A selection whose declared names are the source keys and whose trusted
        digests are a placeholder, for cases that do not vary the digests.
    """
    return release_payload_selection.PayloadSelection(
        declared=tuple(sorted(sources)),
        trusted=dict.fromkeys(sources, "b" * 64),
        sources=sources,
    )


__all__ = ["selection_for", "write_asset", "write_provenance"]
