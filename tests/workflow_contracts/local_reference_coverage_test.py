"""Require reachable local actions and scripts, or reasoned exemptions.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from local_reference_inventory import discover_local_items
from local_reference_test_support import (
    create_synthetic_workspace as _create_workspace,
)
from local_reference_test_support import (
    write_synthetic_file as _write,
)
from local_references import (
    covered_items,
    exemption_issues,
    root_source_texts,
    uncovered_items,
)
from workflow_loading import REPO_ROOT

EXEMPTIONS: dict[str, str] = {}

if typ.TYPE_CHECKING:
    from pathlib import Path

    from local_reference_inventory import LocalItemInventory


@pytest.fixture(scope="module")
def repository_coverage() -> tuple[LocalItemInventory, frozenset[str]]:
    """Collect live repository items and their direct or indirect references."""
    inventory = discover_local_items(REPO_ROOT)
    sources = root_source_texts(REPO_ROOT)
    return inventory, covered_items(inventory, sources)


def test_every_local_item_is_referenced_or_exempt(
    repository_coverage: tuple[LocalItemInventory, frozenset[str]],
) -> None:
    """Require every action and supported script to have an owner."""
    inventory, covered = repository_coverage
    missing = sorted(uncovered_items(inventory, covered, EXEMPTIONS))
    assert not missing, (
        f"unreferenced local actions or scripts: {missing!r}; add a workflow, "
        "Makefile target, or test reference, or add a path-to-reason exemption"
    )


def test_exemptions_have_reasons_and_name_inventory_items(
    repository_coverage: tuple[LocalItemInventory, frozenset[str]],
) -> None:
    """Reject blank exemption reasons and paths outside the inventory."""
    inventory, covered = repository_coverage
    issues = exemption_issues(inventory, frozenset(), EXEMPTIONS)
    assert not issues, "invalid local reference exemptions: " + "; ".join(issues)
    assert covered <= inventory.paths, "coverage must contain inventoried paths only"


def test_exemptions_are_not_stale(
    repository_coverage: tuple[LocalItemInventory, frozenset[str]],
) -> None:
    """Reject an exemption after an item gains a qualifying reference."""
    inventory, covered = repository_coverage
    issues = exemption_issues(inventory, covered, EXEMPTIONS)
    assert not issues, "invalid local reference exemptions: " + "; ".join(issues)


def test_expected_indirect_repository_items_are_covered(
    repository_coverage: tuple[LocalItemInventory, frozenset[str]],
) -> None:
    """Keep known action- and script-mediated tools inside the scan."""
    inventory, covered = repository_coverage
    expected = {
        "scripts/windows-msi-upgrade-cleanup.ps1",
        "scripts/windows-msi-upgrade-validation.ps1",
        "scripts/build-tools-common.sh",
        "scripts/hoist_binstall_discovery.py",
        "scripts/coverage_artifact_archive.py",
        "scripts/doc_coverage_cargo.py",
        "scripts/doc_coverage_model.py",
        "scripts/doc_coverage_runner.py",
    }
    assert expected <= inventory.paths, (
        f"expected indirect items must remain inventoried: "
        f"{sorted(expected - inventory.paths)!r}"
    )
    assert expected <= covered, (
        f"expected indirect items must remain covered: {sorted(expected - covered)!r}"
    )


def test_unreferenced_script_is_reported(tmp_path: Path) -> None:
    """Report a nested script without a workflow, Makefile or test reference."""
    _create_workspace(
        tmp_path,
        "",
        "all:\n\t@true\n",
        {"scripts/helpers/orphan.sh": ""},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))
    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/helpers/orphan.sh"
    }), "an unreferenced nested script must be reported"


def test_inventory_includes_supported_scripts_recursively(
    tmp_path: Path,
) -> None:
    """Include supported scripts at every depth and exclude non-scripts."""
    _create_workspace(
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
            "scripts/__init__.py": "",
            "scripts/readme.json": "",
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
        "scripts/helpers/nested/launch.sh",
        "scripts/tests/test_nested_script.py",
    }, "inventory must include nested scripts and exclude non-script files"


def test_covered_action_and_script_references_reach_a_fixed_point(
    tmp_path: Path,
) -> None:
    """Follow action, shell and bare or package-qualified Python references."""
    workflow = """
name: Synthetic
jobs:
  test:
    steps:
      - uses: ./.github/actions/entry
"""
    action = """
runs:
  using: composite
  steps:
    - shell: bash
      run: bash "$(SCRIPTS)/from-action.sh"
    - shell: bash
      run: python "$(SCRIPTS)/python-caller.py"
    - shell: bash
      run: python "$(SCRIPTS)/package-caller.py"
"""
    scripts = {
        "from-action.sh": 'source "$DIR/from-shell.sh"\n',
        "from-shell.sh": "printf 'reachable'\n",
        "python-caller.py": "from python_helper import main\n",
        "python_helper.py": "def main():\n    return None\n",
        "package-caller.py": (
            "from scripts import package_imported, second_package_imported\n"
            "from scripts import (\n    parenthesized_imported,\n)\n"
            "import scripts.qualified_imported\n"
            "import unrelated_module, scripts.qualified_after_comma, bare_after_comma\n"
        ),
        "package_imported.py": "def main():\n    return None\n",
        "second_package_imported.py": "def main():\n    return None\n",
        "parenthesized_imported.py": "def main():\n    return None\n",
        "qualified_imported.py": "def main():\n    return None\n",
        "qualified_after_comma.py": "def main():\n    return None\n",
        "bare_after_comma.py": "def main():\n    return None\n",
    }
    _create_workspace(
        tmp_path,
        workflow,
        "all:\n\t@true\n",
        {
            ".github/actions/entry/action.yml": action,
            **{f"scripts/{name}": contents for name, contents in scripts.items()},
        },
    )
    inventory = discover_local_items(tmp_path)

    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert covered == inventory.paths, (
        "fixed-point traversal must cover every linked item"
    )


def test_yaml_and_makefile_comments_do_not_count_as_references(
    tmp_path: Path,
) -> None:
    """Ignore YAML and Makefile comments in root and action sources."""
    workflow = """
# uses: ./.github/actions/yaml-comment-only
# run: ./scripts/yaml-comment-only.sh
name: Synthetic
jobs:
  test:
    steps:
      - uses: ./.github/actions/entry
      - run: echo no reference
"""
    makefile = """
# scripts/make-comment-only.sh
all:
\t@true
"""
    _create_workspace(
        tmp_path,
        workflow,
        makefile,
        {
            ".github/actions/entry/action.yml": """# scripts/action-comment-only.sh
runs: {using: composite, steps: []}
""",
            ".github/actions/yaml-comment-only/action.yml": "",
            "scripts/action-comment-only.sh": "",
            "scripts/make-comment-only.sh": "",
            "scripts/yaml-comment-only.sh": "",
        },
    )
    inventory = discover_local_items(tmp_path)

    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert covered == {".github/actions/entry"}, (
        "YAML comments and Makefile comments must not establish references"
    )
    assert uncovered_items(inventory, covered, {}) == frozenset({
        ".github/actions/yaml-comment-only",
        "scripts/action-comment-only.sh",
        "scripts/make-comment-only.sh",
        "scripts/yaml-comment-only.sh",
    }), "items mentioned only in comments must remain uncovered"


def test_test_tree_files_are_root_reference_sources(tmp_path: Path) -> None:
    """Read references from both repository test trees."""
    _create_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t./scripts/tests/test_nested_script.py\n",
        {"scripts/workflow_test.py": "", "scripts/script-test.sh": ""},
    )
    _write(tmp_path, "tests/test_local_script.py", "import workflow_test\n")
    _write(
        tmp_path,
        "scripts/tests/test_nested_script.py",
        'source "$(SCRIPT_DIR)/script-test.sh"\n',
    )
    inventory = discover_local_items(tmp_path)

    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert covered == inventory.paths, "both test trees must establish root references"


def test_script_self_reference_does_not_count_as_coverage(tmp_path: Path) -> None:
    """Keep a script uncovered when its only mention is in its own text."""
    _create_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t@true\n",
        {"scripts/self-referencing.sh": "printf '%s' self-referencing.sh\n"},
    )
    inventory = discover_local_items(tmp_path)

    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert "scripts/self-referencing.sh" not in covered, (
        "a script must not establish its own coverage"
    )
    assert "scripts/self-referencing.sh" in uncovered_items(inventory, covered, {}), (
        "a script referenced only by itself must remain uncovered"
    )


def test_stale_exemption_is_reported(tmp_path: Path) -> None:
    """Reject a reasoned exemption once a Makefile references that script."""
    _create_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t./scripts/referenced.sh\n",
        {"scripts/referenced.sh": ""},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    issues = exemption_issues(
        inventory,
        covered,
        {"scripts/referenced.sh": "Kept while no caller existed."},
    )

    stale_issues = [issue for issue in issues if "stale" in issue]
    assert len(stale_issues) == 1, f"expected one stale diagnostic, got {issues!r}"
    assert "scripts/referenced.sh" in stale_issues[0], (
        f"expected the stale path diagnostic, got {stale_issues!r}"
    )


def test_exemption_needs_a_nonblank_reason_and_inventory_path(tmp_path: Path) -> None:
    """Reject a blank reason and an exemption for a missing local item."""
    _create_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        "all:\n\t@true\n",
        {"scripts/unused.sh": ""},
    )
    inventory = discover_local_items(tmp_path)

    issues = exemption_issues(
        inventory,
        frozenset(),
        {"scripts/unused.sh": "  ", "scripts/unknown.sh": "No such item."},
    )

    assert len(issues) == 2, f"expected two invalid-exemption issues, got {issues!r}"
    assert any("non-blank" in issue for issue in issues), (
        f"expected a blank-reason issue, got {issues!r}"
    )
    assert any("outside the inventory" in issue for issue in issues), (
        f"expected an unknown-path issue, got {issues!r}"
    )
