"""Read test sources used as roots for local-reference reachability."""

import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

from cargo_test_targets import RepositoryFileError
from local_reference_sources import ReferenceSource


def load_test_tree_reference_sources(
    repository_root: Path,
    excluded_paths: frozenset[str],
    source_suffixes: frozenset[str],
) -> tuple[ReferenceSource, ...]:
    """Read supported text sources from both repository test trees.

    Missing test roots and unreadable selected sources raise
    ``RepositoryFileError`` with a path. Individual non-UTF-8 fixtures are
    skipped because binary test data is not source text.

    Parameters
    ----------
    repository_root : Path
        Root of the repository to scan.
    excluded_paths : frozenset[str]
        Repository-relative paths omitted from text scanning.
    source_suffixes : frozenset[str]
        File suffixes that identify readable test sources.

    Returns
    -------
    tuple[ReferenceSource, ...]
        Sources from the ``tests/`` and ``scripts/tests/`` trees.

    """
    sources: list[ReferenceSource] = []
    for directory in _test_source_directories(repository_root):
        for path in _matching_source_paths(directory, source_suffixes):
            relative_path = path.relative_to(repository_root).as_posix()
            if relative_path in excluded_paths:
                continue
            source = _read_test_source(path)
            if source is not None:
                sources.append(ReferenceSource(source, source_suffix=path.suffix))
    return tuple(sources)


def _test_source_directories(repository_root: Path) -> tuple[Path, Path]:
    """Return required source roots, naming a missing or invalid root."""
    directories = (repository_root / "tests", repository_root / "scripts" / "tests")
    for directory in directories:
        if not directory.is_dir():
            message = f"expected a test-source directory at {directory}"
            raise RepositoryFileError(message)
    return directories


def _matching_source_paths(
    directory: Path,
    source_suffixes: frozenset[str],
) -> tuple[Path, ...]:
    """Return supported regular text files below one test root."""
    try:
        paths = sorted(directory.rglob("*"))
    except OSError as error:
        message = f"cannot scan test-source directory {directory}: {error}"
        raise RepositoryFileError(message) from error
    return tuple(path for path in paths if _is_test_source(path, source_suffixes))


def _is_test_source(path: Path, source_suffixes: frozenset[str]) -> bool:
    """Keep regular supported files and reject caches or symlinks."""
    if not path.is_file() or path.is_symlink():
        return False
    if "__pycache__" in path.parts or path.suffix == ".pyc":
        return False
    return path.suffix in source_suffixes


def _read_test_source(path: Path) -> str | None:
    """Read one test source, skipping undecodable fixtures by design."""
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return None
    except OSError as error:
        message = f"cannot read test source {path}: {error}"
        raise RepositoryFileError(message) from error
