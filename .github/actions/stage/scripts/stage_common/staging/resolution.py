"""Path resolution helpers for artefact staging."""

from __future__ import annotations

import glob
import typing as typ
from pathlib import Path, PurePosixPath, PureWindowsPath

__all__ = ["_match_candidate_path"]


def _match_candidate_path(workspace: Path, rendered: str) -> Path | None:
    """Return the newest path matching ``rendered`` relative to ``workspace``."""

    def _newest_file(candidates: typ.Iterable[Path]) -> Path | None:
        best_path: Path | None = None
        best_key: tuple[int, str] | None = None
        for candidate in candidates:
            path = Path(candidate)
            if not path.is_file():
                continue
            try:
                key = (int(path.stat().st_mtime_ns), path.as_posix())
            except OSError:
                key = (0, path.as_posix())
            if best_key is None or key > best_key:
                best_key = key
                best_path = path
        return best_path

    def _windows_root() -> tuple[Path, tuple[str, ...]]:
        windows_candidate = PureWindowsPath(rendered)
        root = Path(windows_candidate.anchor or "/")
        relative_parts = windows_candidate.parts[1:]
        return root, relative_parts

    candidate = Path(rendered)
    if glob.has_magic(rendered):
        if candidate.is_absolute():
            root = Path(candidate.anchor or "/")
            relative_parts = candidate.parts[1:]
            pattern = (
                PurePosixPath(*relative_parts).as_posix() if relative_parts else "*"
            )
            matches = root.glob(pattern)
        elif PureWindowsPath(rendered).is_absolute():
            root, relative_parts = _windows_root()
            pattern = (
                PurePosixPath(*relative_parts).as_posix()
                if relative_parts
                else "*"
            )
            matches = root.glob(pattern)
        else:
            matches = workspace.glob(rendered)
        return _newest_file(matches)

    if candidate.is_absolute():
        base = candidate
    elif PureWindowsPath(rendered).is_absolute():
        root, relative_parts = _windows_root()
        base = root.joinpath(*relative_parts)
    else:
        base = workspace / candidate
    return base if base.is_file() else None
