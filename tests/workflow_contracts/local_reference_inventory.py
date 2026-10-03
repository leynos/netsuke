"""Inventory local composite actions and scripts for reference contracts.

Keep repository item discovery separate from reference parsing so the
reachability contract can work with a small, explicit inventory.
"""

import dataclasses
import typing as typ

from cargo_test_targets import RepositoryFileError

if typ.TYPE_CHECKING:
    from pathlib import Path

ACTION_DIRECTORY = ".github/actions"
SCRIPT_SUFFIXES = frozenset({".bash", ".py", ".ps1", ".sh"})


@dataclasses.dataclass(frozen=True, slots=True)
class LocalItemInventory:
    """Map each local action or script path to the file that defines it."""

    action_manifests: dict[str, tuple[Path, ...]]
    scripts: dict[str, Path]

    @property
    def paths(self) -> frozenset[str]:
        """Repository-relative paths for all inventoried items."""
        return frozenset(self.action_manifests) | frozenset(self.scripts)


def discover_local_items(repository_root: Path) -> LocalItemInventory:
    """Inventory action directories and supported scripts recursively.

    Action directories need an ``action.yml`` or ``action.yaml`` manifest.
    Scripts are regular files at any depth under ``scripts/`` with a supported
    suffix. Package markers, data files, bytecode caches and symlinks are
    excluded because they are not executable script entry points.

    Parameters
    ----------
    repository_root : Path
        Root of the repository to scan.

    Returns
    -------
    LocalItemInventory
        The action manifests and scripts found under the repository root.
    """
    return LocalItemInventory(
        _discover_action_manifests(repository_root),
        _discover_scripts(repository_root),
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
    """Return supported regular script files at any depth under ``scripts/``."""
    scripts_root = repository_root / "scripts"
    directories = [scripts_root]
    scripts: dict[str, Path] = {}
    while directories:
        directory = directories.pop()
        for path in _inventory_directory_entries(directory):
            if path.is_symlink():
                continue
            if path.is_dir():
                if path.name != "__pycache__":
                    directories.append(path)
            elif _is_supported_script(path):
                scripts[path.relative_to(repository_root).as_posix()] = path
    return dict(sorted(scripts.items()))


def _is_supported_script(path: Path) -> bool:
    """Check whether a path names a supported regular script file."""
    if path.name == "__init__.py" or path.suffix not in SCRIPT_SUFFIXES:
        return False
    return path.is_file() and not path.is_symlink()


def _inventory_directory_entries(directory: Path) -> tuple[Path, ...]:
    """List one required inventory root, naming and chaining listing errors."""
    try:
        return tuple(sorted(directory.iterdir()))
    except OSError as error:
        message = f"cannot list local inventory directory {directory}: {error}"
        raise RepositoryFileError(message) from error
