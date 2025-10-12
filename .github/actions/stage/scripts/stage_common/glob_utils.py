"""Path matching helpers for artefact discovery."""

from __future__ import annotations

from glob import has_magic
from pathlib import Path, PurePath, PurePosixPath, PureWindowsPath

__all__ = ["glob_root_and_pattern", "match_candidate_path"]


def match_candidate_path(workspace: Path, rendered: str) -> Path | None:
    """Return the newest path matching ``rendered`` relative to ``workspace``."""
    candidate = Path(rendered)
    if not has_magic(rendered):
        base = candidate if candidate.is_absolute() else workspace / candidate
        return base if base.is_file() else None

    candidates = _resolve_glob_candidates(workspace, candidate, rendered)
    return _select_newest_or_none(candidates)


def _resolve_glob_candidates(
    workspace: Path, candidate: Path, rendered: str
) -> list[Path]:
    if candidate.is_absolute():
        return _resolve_absolute_glob(candidate)

    windows_candidate = PureWindowsPath(rendered)
    if windows_candidate.is_absolute():
        return _resolve_windows_absolute_glob(windows_candidate)

    return _resolve_relative_glob(workspace, rendered)


def _resolve_absolute_glob(candidate: Path) -> list[Path]:
    root_text, pattern = glob_root_and_pattern(candidate)
    root = Path(root_text)
    return [path for path in root.glob(pattern) if path.is_file()]


def _resolve_windows_absolute_glob(
    windows_candidate: PureWindowsPath,
) -> list[Path]:
    anchor = Path(windows_candidate.anchor)
    relative = PureWindowsPath(*windows_candidate.parts[1:]).as_posix()
    return [path for path in anchor.glob(relative) if path.is_file()]


def _resolve_relative_glob(workspace: Path, rendered: str) -> list[Path]:
    return [path for path in workspace.glob(rendered) if path.is_file()]


def _select_newest_or_none(candidates: list[Path]) -> Path | None:
    return max(candidates, key=_mtime_key) if candidates else None


def glob_root_and_pattern(candidate: PurePath) -> tuple[str, str]:
    """Return the filesystem root and relative glob pattern for ``candidate``."""
    anchor = candidate.anchor
    if not anchor:
        message = f"Expected absolute path, received '{candidate}'"
        raise ValueError(message)

    root_text = (candidate.drive + candidate.root) or anchor or "/"
    relative_parts = candidate.parts[1:]
    pattern = PurePosixPath(*relative_parts).as_posix() if relative_parts else "*"
    return root_text, pattern


def _mtime_key(path: Path) -> tuple[int, str]:
    """Return a sortable key derived from ``path``'s modification time."""
    try:
        return (int(path.stat().st_mtime_ns), path.as_posix())
    except OSError:  # pragma: no cover - filesystem race guard.
        return (0, path.as_posix())
