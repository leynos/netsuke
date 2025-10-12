"""Path resolution helpers for artefact staging."""

from __future__ import annotations

import glob
import typing as typ
from pathlib import Path, PurePosixPath, PureWindowsPath

__all__ = ["_match_candidate_path"]


def _newest_file(candidates: typ.Iterable[Path]) -> Path | None:
    """Return the newest file from ``candidates``.

    Example:
        >>> tmp = Path("/tmp/example")
        >>> tmp.mkdir(exist_ok=True)
        >>> sample = tmp / "sample.txt"
        >>> sample.write_text("payload", encoding="utf-8")
        >>> _newest_file([sample]) == sample
        True
    """

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


def _windows_root(rendered: str) -> tuple[Path, tuple[str, ...]]:
    """Return the Windows drive root and path components for ``rendered``.

    Example:
        >>> _windows_root("C:/workspace/app.exe")[0]
        PosixPath('C:/')
    """

    windows_candidate = PureWindowsPath(rendered)
    root = Path(windows_candidate.anchor or "/")
    relative_parts = windows_candidate.parts[1:]
    return root, relative_parts


def _resolve_glob_pattern(
    workspace: Path, rendered: str, candidate: Path
) -> Path | None:
    """Resolve a glob ``rendered`` against ``workspace``.

    Example:
        >>> workspace = Path("/tmp/work")
        >>> (workspace / "dist").mkdir(parents=True, exist_ok=True)
        >>> target = workspace / "dist" / "netsuke.txt"
        >>> target.write_text("payload", encoding="utf-8")
        >>> _resolve_glob_pattern(workspace, "dist/*.txt", Path("dist/*.txt"))
        PosixPath('/tmp/work/dist/netsuke.txt')
    """

    if candidate.is_absolute():
        root = Path(candidate.anchor or "/")
        relative_parts = candidate.parts[1:]
        pattern = PurePosixPath(*relative_parts).as_posix() if relative_parts else "*"
        matches = root.glob(pattern)
    elif PureWindowsPath(rendered).is_absolute():
        root, relative_parts = _windows_root(rendered)
        pattern = (
            PurePosixPath(*relative_parts).as_posix() if relative_parts else "*"
        )
        matches = root.glob(pattern)
    else:
        matches = workspace.glob(rendered)
    return _newest_file(matches)


def _resolve_direct_path(
    workspace: Path, rendered: str, candidate: Path
) -> Path | None:
    """Resolve a direct ``rendered`` path relative to ``workspace``.

    Example:
        >>> workspace = Path("/tmp/work")
        >>> workspace.mkdir(parents=True, exist_ok=True)
        >>> target = workspace / "netsuke.txt"
        >>> target.write_text("payload", encoding="utf-8")
        >>> _resolve_direct_path(workspace, "netsuke.txt", Path("netsuke.txt"))
        PosixPath('/tmp/work/netsuke.txt')
    """

    if candidate.is_absolute():
        base = candidate
    elif PureWindowsPath(rendered).is_absolute():
        root, relative_parts = _windows_root(rendered)
        base = root.joinpath(*relative_parts)
    else:
        base = workspace / candidate
    return base if base.is_file() else None


def _match_candidate_path(workspace: Path, rendered: str) -> Path | None:
    """Return the newest path matching ``rendered`` relative to ``workspace``.

    Example:
        >>> workspace = Path("/tmp/work")
        >>> workspace.mkdir(parents=True, exist_ok=True)
        >>> (workspace / "dist").mkdir(exist_ok=True)
        >>> target = workspace / "dist" / "netsuke.txt"
        >>> target.write_text("payload", encoding="utf-8")
        >>> _match_candidate_path(workspace, "dist/*.txt") == target
        True
    """

    candidate = Path(rendered)
    resolver = (
        _resolve_glob_pattern if glob.has_magic(rendered) else _resolve_direct_path
    )
    return resolver(workspace, rendered, candidate)
