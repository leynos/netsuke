"""Provide cache support types and atomic writes for the spelling helper."""

from __future__ import annotations

import dataclasses as dc
import pathlib
import tempfile
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


@dc.dataclass(frozen=True)
class RefreshResult:
    """Describe whether the untracked shared dictionary cache changed."""

    status: str
    cache: pathlib.Path


@dc.dataclass(frozen=True)
class CacheTargets:
    """Group the untracked dictionary cache and metadata sidecar paths."""

    cache: pathlib.Path
    metadata: pathlib.Path


class RemoteResponse(typ.Protocol):
    """Expose the HTTP response surface used by cache refresh."""

    status: int
    headers: cabc.Mapping[str, str]

    def read(self) -> bytes:
        """Read the response body."""
        ...


def atomic_write(path: pathlib.Path, content: bytes) -> None:
    """Write content beside a path and atomically replace the destination.

    Parameters
    ----------
    path
        Destination path to replace.
    content
        Complete bytes to install.

    Notes
    -----
    The temporary file is created beside the destination, then replaced on the
    same filesystem. Cleanup removes the temporary path after every outcome.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    stream = tempfile.NamedTemporaryFile(
        delete=False, dir=path.parent, prefix=f".{path.name}."
    )
    temporary = pathlib.Path(stream.name)
    try:
        with stream:
            stream.write(content)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
