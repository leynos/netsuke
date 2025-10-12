"""Checksum helpers for staged artefacts."""

from __future__ import annotations

import hashlib
from pathlib import Path

__all__ = ["write_checksum"]


def write_checksum(path: Path, algorithm: str) -> str:
    """Write the checksum sidecar for ``path`` using ``algorithm``.

    Parameters
    ----------
    path:
        Path to the file whose contents should be hashed.
    algorithm:
        Hashing algorithm name supported by :mod:`hashlib` (for example
        ``"sha256"``).

    Returns
    -------
    str
        Hex digest generated for ``path`` using ``algorithm``.
    """

    hasher = hashlib.new(algorithm)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            hasher.update(chunk)
    digest = hasher.hexdigest()
    checksum_path = path.with_name(f"{path.name}.{algorithm}")
    checksum_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return digest
