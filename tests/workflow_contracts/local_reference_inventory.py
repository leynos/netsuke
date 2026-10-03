"""Inventory local composite actions and scripts for reference contracts.

Keep repository item discovery separate from reference parsing so the
reachability contract can work with a small, explicit inventory.
"""

import dataclasses
import stat
import typing as typ

from cargo_test_targets import RepositoryFileError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

ACTION_DIRECTORY = ".github/actions"
SCRIPT_SUFFIXES = frozenset({".bash", ".py", ".ps1", ".sh"})


@dataclasses.dataclass(frozen=True, slots=True)
class LocalItemInventory:
    """Map each local action or script path to the file that defines it."""

    action_manifests: dict[str, tuple[Path, ...]]
    scripts: dict[str, Path]
    unclassified_top_level_files: dict[str, Path]

    @property
    def paths(self) -> frozenset[str]:
        """Repository-relative paths for all inventoried items."""
        return (
            frozenset(self.action_manifests)
            | frozenset(self.scripts)
            | frozenset(self.unclassified_top_level_files)
        )


def discover_local_items(repository_root: Path) -> LocalItemInventory:
    """Inventory action directories and supported scripts recursively.

    Action directories need an ``action.yml`` or ``action.yaml`` manifest.
    Scripts are regular files at any depth under ``scripts/`` with a known
    suffix, executable bit, or shebang. Unsupported direct files are retained
    as unclassified items, so the contract requires a reasoned exemption.
    Package markers, bytecode caches and symlinks are excluded from script
    discovery.

    Parameters
    ----------
    repository_root : Path
        Root of the repository to scan.

    Returns
    -------
    LocalItemInventory
        The action manifests and scripts found under the repository root.
    """
    scripts = _discover_scripts(repository_root)
    unclassified = _discover_unclassified_top_level_files(repository_root, scripts)
    return LocalItemInventory(
        _discover_action_manifests(repository_root), scripts, unclassified
    )


def _discover_action_manifests(repository_root: Path) -> dict[str, tuple[Path, ...]]:
    """Return manifests grouped by their local action directory."""
    action_root = repository_root / ACTION_DIRECTORY
    action_manifests: dict[str, tuple[Path, ...]] = {}
    for directory in _inventory_directory_entries(action_root):
        if not directory.is_dir() or directory.is_symlink():
            continue
        manifests = tuple(
            path
            for path in (directory / "action.yml", directory / "action.yaml")
            if path.is_file() and not path.is_symlink()
        )
        if manifests:
            item_path = directory.relative_to(repository_root).as_posix()
            action_manifests[item_path] = manifests
    return action_manifests


def _discover_scripts(repository_root: Path) -> dict[str, Path]:
    """Return regular scripts identified by suffix, mode, or shebang."""
    scripts_root = repository_root / "scripts"
    scripts = {
        path.relative_to(repository_root).as_posix(): path
        for path in _iter_script_files(scripts_root)
    }
    return dict(sorted(scripts.items()))


def _iter_script_files(scripts_root: Path) -> cabc.Iterator[Path]:
    """Yield supported scripts without following links or entering caches."""
    directories = [scripts_root]
    while directories:
        directory = directories.pop()
        entries = _inventory_directory_entries(directory)
        directories.extend(
            path
            for path in entries
            if not path.is_symlink()
            if path.is_dir()
            if path.name != "__pycache__"
        )
        yield from (path for path in entries if _is_supported_script(path))


def _is_supported_script(path: Path) -> bool:
    """Check whether a regular file looks like a script entry point."""
    if path.name == "__init__.py":
        return False
    if path.is_symlink():
        return False
    if not path.is_file():
        return False
    return (
        path.suffix in SCRIPT_SUFFIXES
        or bool(path.stat().st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))
        or _has_shebang(path)
    )


def _has_shebang(path: Path) -> bool:
    """Identify a script from its interpreter marker on the first line."""
    try:
        with path.open("rb") as source:
            return source.readline().startswith(b"#!")
    except OSError as error:
        message = f"cannot inspect local script {path}: {error}"
        raise RepositoryFileError(message) from error


def _discover_unclassified_top_level_files(
    repository_root: Path,
    scripts: dict[str, Path],
) -> dict[str, Path]:
    """Return direct files that need a reference or an explicit exemption."""
    scripts_root = repository_root / "scripts"
    classified = set(scripts)
    unclassified: dict[str, Path] = {}
    for path in _inventory_directory_entries(scripts_root):
        if path.is_dir() and not path.is_symlink():
            continue
        relative_path = path.relative_to(repository_root).as_posix()
        if relative_path not in classified:
            unclassified[relative_path] = path
    return dict(sorted(unclassified.items()))


def _inventory_directory_entries(directory: Path) -> tuple[Path, ...]:
    """List one required inventory root, naming and chaining listing errors."""
    try:
        return tuple(sorted(directory.iterdir()))
    except OSError as error:
        message = f"cannot list local inventory directory {directory}: {error}"
        raise RepositoryFileError(message) from error
