"""Glob and path resolution helpers for artefact staging."""

from __future__ import annotations

import glob
import typing as typ
from pathlib import Path, PurePath, PurePosixPath, PureWindowsPath

__all__ = [
    "_contains_glob",
    "_glob_root_and_pattern",
    "_iter_absolute_matches",
    "_match_candidate_path",
    "_match_glob_candidate",
    "_mtime_key",
    "_newest_file",
]


def _match_candidate_path(workspace: Path, rendered: str) -> Path | None:
    """Return the newest path matching ``rendered`` relative to ``workspace``."""

    candidate = Path(rendered)
    if _contains_glob(rendered):
        return _match_glob_candidate(workspace, candidate, rendered)

    base = candidate if candidate.is_absolute() else workspace / candidate
    return base if base.is_file() else None


def _match_glob_candidate(
    workspace: Path, candidate: Path, rendered: str
) -> Path | None:
    """Resolve the newest match for a glob ``rendered``.

    Examples
    --------
    >>> workspace = Path('.')
    >>> candidate = workspace / 'dist'
    >>> candidate.mkdir(exist_ok=True)
    >>> file = candidate / 'netsuke.txt'
    >>> _ = file.write_text('payload', encoding='utf-8')
    >>> _match_glob_candidate(workspace, Path('dist/*.txt'), 'dist/*.txt') == file
    True
    """

    if candidate.is_absolute():
        matches = _iter_absolute_matches(candidate)
    else:
        windows_candidate = PureWindowsPath(rendered)
        if windows_candidate.is_absolute():
            matches = _iter_absolute_matches(windows_candidate)
        else:
            matches = workspace.glob(rendered)
    return _newest_file(matches)


def _contains_glob(pattern: str) -> bool:
    """Return ``True`` when ``pattern`` contains glob wildcards."""

    return glob.has_magic(pattern)


def _iter_absolute_matches(candidate: PurePath) -> typ.Iterable[Path]:
    """Yield files for an absolute glob ``candidate`` on any platform."""

    root_text, pattern = _glob_root_and_pattern(candidate)
    root = Path(root_text)
    return root.glob(pattern)


def _newest_file(candidates: typ.Iterable[Path]) -> Path | None:
    """Return the newest file from ``candidates`` (if any)."""

    files = [path for path in candidates if path.is_file()]
    return max(files, key=_mtime_key) if files else None


def _mtime_key(path: Path) -> tuple[int, str]:
    """Provide a sortable key using mtime with a stable tie-breaker (e.g., ``_mtime_key(Path('file'))`` -> ``(123, 'file')``)."""

    try:
        return (int(path.stat().st_mtime_ns), path.as_posix())
    except OSError:
        return (0, path.as_posix())


def _glob_root_and_pattern(candidate: PurePath) -> tuple[str, str]:
    """Return the filesystem root and relative glob pattern for ``candidate``."""

    anchor = candidate.anchor
    if not anchor:
        message = f"Expected absolute path, received '{candidate}'"
        raise ValueError(message)

    root_text = (candidate.drive + candidate.root) or anchor or "/"
    relative_parts = candidate.parts[1:]
    pattern = (
        PurePosixPath(*relative_parts).as_posix() if relative_parts else "*"
    )
    return root_text, pattern

