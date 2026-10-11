"""Test recursive script classification and fail-closed inventory entries.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from local_reference_inventory import discover_local_items
from local_reference_test_support import (
    create_synthetic_workspace,
    write_synthetic_file,
)
from local_references import covered_items, root_source_texts, uncovered_items

if typ.TYPE_CHECKING:
    from pathlib import Path


def test_inventory_includes_supported_scripts_recursively(tmp_path: Path) -> None:
    """Include suffix and shebang scripts at every depth."""
    create_synthetic_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t@true\n",
        {
            ".github/actions/yml-action/action.yml": "",
            ".github/actions/yaml-action/action.yaml": "",
            "scripts/entry.sh": "",
            "scripts/entry.bash": "",
            "scripts/entry.py": "",
            "scripts/entry.ps1": "",
            "scripts/entry-no-suffix": "#!/usr/bin/env bash\ntrue\n",
            "scripts/__init__.py": "",
            "scripts/readme.json": "",
            "scripts/unclassified.data": "",
            "scripts/helpers/nested/launch.sh": "",
            "scripts/tests/test_nested_script.py": "",
            "scripts/helpers/__init__.py": "",
            "scripts/tests/data/fixture.json": "",
            "scripts/__pycache__/ignored.pyc": "",
            "scripts/__pycache__/ignored.py": "",
        },
    )

    inventory = discover_local_items(tmp_path)

    assert inventory.action_manifests.keys() == {
        ".github/actions/yml-action",
        ".github/actions/yaml-action",
    }, "inventory must include both supported action manifest extensions"
    assert inventory.scripts.keys() == {
        "scripts/entry.sh",
        "scripts/entry.bash",
        "scripts/entry.py",
        "scripts/entry.ps1",
        "scripts/entry-no-suffix",
        "scripts/helpers/nested/launch.sh",
        "scripts/tests/test_nested_script.py",
    }, "inventory must include nested scripts and exclude ordinary data files"
    assert inventory.unclassified_top_level_files.keys() == {
        "scripts/__init__.py",
        "scripts/readme.json",
        "scripts/unclassified.data",
    }, "unknown direct files must remain visible to the exemption contract"


def test_unclassified_top_level_file_requires_a_reasoned_exemption(
    tmp_path: Path,
) -> None:
    """Require a path-specific reason for an unknown top-level file."""
    create_synthetic_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t@true\n",
        {"scripts/notes.data": "not executable data\n"},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/notes.data"
    }), "an unclassified top-level item must fail closed"
    assert not uncovered_items(
        inventory,
        covered,
        {"scripts/notes.data": "Fixture consumed only by local tooling."},
    ), "a non-blank path-specific reason must allow the explicit exemption"


def test_shebang_scripts_exclude_package_markers(tmp_path: Path) -> None:
    """Include suffixless shebangs while excluding Python package markers."""
    create_synthetic_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t@true\n",
        {"scripts/nested/__init__.py": "#!/bin/sh\n"},
    )
    write_synthetic_file(tmp_path, "scripts/nested/no_suffix", "#!/bin/sh\n")

    inventory = discover_local_items(tmp_path)

    assert inventory.scripts.keys() == {"scripts/nested/no_suffix"}, (
        "a package marker remains excluded even when it has a shebang"
    )
