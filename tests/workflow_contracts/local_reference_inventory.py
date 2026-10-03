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
    """Inventory action directories and directly contained script files.

    Action directories need an ``action.yml`` or ``action.yaml`` manifest.
    Scripts must be regular files directly in ``scripts/`` with a supported
    suffix; nested tests, data, bytecode caches and ``__init__.py`` are not
    executable top-level scripts and are excluded.

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
        _discover_top_level_scripts(repository_root),
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


def _discover_top_level_scripts(repository_root: Path) -> dict[str, Path]:
    """Return supported regular files directly inside ``scripts/``."""
    scripts_root = repository_root / "scripts"
    return {
        path.relative_to(repository_root).as_posix(): path
        for path in _inventory_directory_entries(scripts_root)
        if path.suffix in SCRIPT_SUFFIXES
        and path.name != "__init__.py"
        and path.is_file()
        and not path.is_symlink()
    }


def _inventory_directory_entries(directory: Path) -> tuple[Path, ...]:
    """List one required inventory root, naming and chaining listing errors."""
    try:
        return tuple(sorted(directory.iterdir()))
    except OSError as error:
        message = f"cannot list local inventory directory {directory}: {error}"
        raise RepositoryFileError(message) from error
