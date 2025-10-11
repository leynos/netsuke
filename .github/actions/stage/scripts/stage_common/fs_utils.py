"""Filesystem helpers for staging."""

from __future__ import annotations

from pathlib import Path

from .errors import StageError

__all__ = ["safe_destination_path"]


def safe_destination_path(staging_dir: Path, destination: str) -> Path:
    """Return ``destination`` resolved beneath ``staging_dir``."""

    target = (staging_dir / destination).resolve()
    staging_root = staging_dir.resolve()
    if not target.is_relative_to(staging_root):
        message = f"Destination escapes staging directory: {destination}"
        raise StageError(message)
    target.parent.mkdir(parents=True, exist_ok=True)
    return target
