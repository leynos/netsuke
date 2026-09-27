"""Keep Rust source modules grouped by their shared name prefix.

The check owns the layout convention under ``src/``. An exception must name
the exact sibling files or directory modules and explain why their common
prefix is coincidental.
Run through ``make test-workflow-contracts``.
"""

from collections import defaultdict
from itertools import chain
from pathlib import Path

import pytest

SOURCE_ROOT = Path(__file__).resolve().parents[2] / "src"
IGNORED_MODULE_NAMES = frozenset({"lib", "main", "mod"})

# Keyed by the directory relative to src/ and the shared first prefix.
# Every entry must carry the exact sibling set and a disparate-concern rationale.
DISPARATE_PREFIX_EXCEPTIONS: dict[tuple[str, str], tuple[frozenset[str], str]] = {}


def sibling_prefix_groups(source_root: Path) -> dict[tuple[str, str], frozenset[str]]:
    """Find sibling Rust files or directory modules sharing a first prefix.

    Parameters
    ----------
    source_root : Path
        Root of the Rust source tree to inspect.

    Returns
    -------
    dict[tuple[str, str], frozenset[str]]
        Exact sibling names keyed by relative directory and prefix. A trailing
        slash identifies a directory module.
    """
    groups: dict[tuple[str, str], set[str]] = defaultdict(set)
    file_modules = (
        (source.parent, source.stem, source.name)
        for source in source_root.rglob("*.rs")
        if source.stem not in IGNORED_MODULE_NAMES
    )
    directory_modules = (
        (
            module_root.parent.parent,
            module_root.parent.name,
            f"{module_root.parent.name}/",
        )
        for module_root in source_root.rglob("mod.rs")
        if module_root.parent != source_root
    )
    for parent, module_name, sibling_name in chain(file_modules, directory_modules):
        directory = parent.relative_to(source_root).as_posix()
        prefix = module_name.partition("_")[0]
        groups[directory, prefix].add(sibling_name)
    return {
        key: frozenset(filenames)
        for key, filenames in groups.items()
        if len(filenames) > 1
    }


def prefixed_files_beside_directory(source_root: Path) -> list[str]:
    """Find prefixed sibling files left beside their directory module.

    Parameters
    ----------
    source_root : Path
        Root of the Rust source tree to inspect.

    Returns
    -------
    list[str]
        Sorted relative paths to files beside a matching directory module.
    """
    violations = []
    for module_root in source_root.rglob("mod.rs"):
        directory = module_root.parent
        parent = directory.parent
        violations.extend(
            sibling.relative_to(source_root).as_posix()
            for sibling in parent.glob(f"{directory.name}_*.rs")
        )
    return sorted(violations)


def assert_module_layout(
    source_root: Path,
    exceptions: dict[tuple[str, str], tuple[frozenset[str], str]],
) -> None:
    """Reject new prefix groups, stale exceptions, and adjacent prefixed files.

    Parameters
    ----------
    source_root : Path
        Root of the Rust source tree to inspect.
    exceptions : dict[tuple[str, str], tuple[frozenset[str], str]]
        Exact sibling names and rationale for each coincidental prefix.

    Notes
    -----
    An assertion failure lists missing or stale exceptions and misplaced
    prefixed files.
    """
    groups = sibling_prefix_groups(source_root)
    errors = []
    for key, filenames in sorted(groups.items()):
        exception = exceptions.get(key)
        if exception is None:
            errors.append(f"unrecorded prefix group {key}: {sorted(filenames)}")
        elif exception[0] != filenames or not exception[1].strip():
            errors.append(f"stale or unjustified prefix exception {key}")
    errors.extend(
        f"stale prefix exception {key}"
        for key in sorted(exceptions.keys() - groups.keys())
    )
    errors.extend(
        f"prefixed sibling beside directory module: {sibling}"
        for sibling in prefixed_files_beside_directory(source_root)
    )
    assert not errors, "\n".join(errors)


def test_repository_modules_follow_directory_layout() -> None:
    """Keep the live source tree free of unrecorded prefix groups."""
    assert_module_layout(SOURCE_ROOT, DISPARATE_PREFIX_EXCEPTIONS)


def test_added_prefix_pair_fails_contract(tmp_path: Path) -> None:
    """Prove a newly added module-plus-test sibling trips the guard."""
    source_root = tmp_path / "src"
    source_root.mkdir()
    (source_root / "widget.rs").touch()
    (source_root / "widget_tests.rs").touch()
    with pytest.raises(AssertionError, match="unrecorded prefix group"):
        assert_module_layout(source_root, DISPARATE_PREFIX_EXCEPTIONS)


def test_added_prefix_directory_pair_fails_contract(tmp_path: Path) -> None:
    """Prove sibling directory modules with a shared prefix trip the guard."""
    source_root = tmp_path / "src"
    for name in ("widget", "widget_detail"):
        directory = source_root / name
        directory.mkdir(parents=True)
        (directory / "mod.rs").touch()
    with pytest.raises(AssertionError, match="unrecorded prefix group"):
        assert_module_layout(source_root, DISPARATE_PREFIX_EXCEPTIONS)


def test_prefixed_file_beside_directory_fails_contract(tmp_path: Path) -> None:
    """Prove one misplaced file fails even without a sibling prefix pair."""
    source_root = tmp_path / "src"
    directory = source_root / "widget"
    directory.mkdir(parents=True)
    (directory / "mod.rs").touch()
    (source_root / "widget_error.rs").touch()
    with pytest.raises(AssertionError, match="prefixed sibling beside directory"):
        assert_module_layout(source_root, DISPARATE_PREFIX_EXCEPTIONS)


def test_exception_requires_exact_files_and_reason(tmp_path: Path) -> None:
    """Prevent an allowlist entry from masking a changed prefix group."""
    source_root = tmp_path / "src"
    source_root.mkdir()
    (source_root / "widget_a.rs").touch()
    (source_root / "widget_b.rs").touch()
    exception = {(".", "widget"): (frozenset({"widget_a.rs"}), "distinct concerns")}
    with pytest.raises(AssertionError, match="stale or unjustified"):
        assert_module_layout(source_root, exception)
