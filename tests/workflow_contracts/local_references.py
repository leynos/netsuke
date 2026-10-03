"""Find local actions and scripts reachable from repository entry points.

Workflow, Makefile and test sources can call a script directly, while a
covered action or script can introduce further references. This module
collects those references to a fixed point so only unreferenced
release helpers need an exemption.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import re
import typing as typ
from pathlib import Path

from cargo_test_targets import read_repository_file
from local_reference_comments import strip_reference_comments
from local_reference_sources import (
    ReferenceSource,
    relative_script_is_referenced,
    scripts_package_imports_module,
    workflow_reference_sources,
)
from local_reference_test_trees import load_test_tree_reference_sources
from workflow_loading import all_workflow_documents, read_workflow_document

if typ.TYPE_CHECKING:
    from local_reference_inventory import LocalItemInventory

TEST_SOURCE_SUFFIXES = frozenset({
    ".feature",
    ".json",
    ".jsonl",
    ".md",
    ".ps1",
    ".proptest-regressions",
    ".py",
    ".rs",
    ".sh",
    ".snap",
    ".toml",
    ".txt",
    ".yml",
    ".yaml",
})
CONTRACT_SOURCE_PATHS = frozenset({
    "tests/workflow_contracts/local_references.py",
    "tests/workflow_contracts/local_reference_coverage_test.py",
    "tests/workflow_contracts/local_reference_edge_cases_test.py",
    "tests/workflow_contracts/local_reference_inventory.py",
    "tests/workflow_contracts/local_reference_inventory_test.py",
    "tests/workflow_contracts/local_reference_property_test.py",
    "tests/workflow_contracts/local_reference_comments.py",
    "tests/workflow_contracts/local_reference_sources.py",
    "tests/workflow_contracts/local_reference_test_trees.py",
    "tests/workflow_contracts/local_reference_test_support.py",
})


def strip_makefile_comments(text: str) -> str:
    r"""Remove full-line Makefile comments before checking commands.

    >>> strip_makefile_comments("# scripts/ignored.sh\nrun: scripts/used.sh\n")
    'run: scripts/used.sh'

    Parameters
    ----------
    text : str
        Makefile contents to filter.

    Returns
    -------
    str
        Makefile text with full-line comments removed.
    """
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def root_source_texts(
    repository_root: Path,
    excluded_paths: frozenset[str] = CONTRACT_SOURCE_PATHS,
) -> tuple[ReferenceSource, ...]:
    """Read workflow strings, Makefile commands, and test source files.

    Workflow documents are parsed before scanning, so YAML comments do not
    count as references. Full-line Makefile comments are removed, and the
    reference contract excludes its own implementation and assertions. Only
    known text-source suffixes are read; undecodable test fixtures are skipped.
    Both test source roots must exist, and other read failures name their path.

    Parameters
    ----------
    repository_root : Path
        Root of the repository to scan.
    excluded_paths : frozenset[str]
        Repository-relative paths omitted from text scanning.

    Returns
    -------
    tuple[ReferenceSource, ...]
        The text sources used to find references to inventoried items.

    """
    workflow_directory = repository_root / ".github" / "workflows"
    workflows = all_workflow_documents(workflow_directory)
    sources = [
        source
        for document in workflows.values()
        for source in workflow_reference_sources(document)
    ]

    makefile = repository_root / "Makefile"
    sources.append(
        ReferenceSource(strip_makefile_comments(read_repository_file(makefile)))
    )

    sources.extend(
        load_test_tree_reference_sources(
            repository_root, excluded_paths, TEST_SOURCE_SUFFIXES
        )
    )
    return tuple(sources)


def referenced_items(
    source: ReferenceSource,
    inventory: LocalItemInventory,
    source_item: str | None = None,
) -> frozenset[str]:
    """Return inventoried items named by one source, excluding itself.

    Action references use their local ``./.github/actions/<name>`` path.
    Script references use the file name at a word or path boundary, with
    Python module imports recognised for ``.py`` files.

    Parameters
    ----------
    source : ReferenceSource
        Source text and the context needed to interpret it.
    inventory : LocalItemInventory
        Local actions and scripts that may be referenced.
    source_item : str or None
        Item whose own text must not establish its coverage.

    Returns
    -------
    frozenset[str]
        Paths of referenced local actions and scripts.
    """
    source_suffix = source.script_suffix or source.source_suffix
    source = dataclasses.replace(
        source, text=strip_reference_comments(source.text, source_suffix)
    )
    text = source.text
    action_references = {
        item_path
        for item_path in inventory.action_manifests
        if item_path != source_item and _action_is_referenced(text, item_path)
    }
    script_references = {
        item_path
        for item_path in inventory.scripts
        if item_path != source_item and _script_is_referenced(source, item_path)
    }
    return frozenset(action_references | script_references)


def covered_items(
    inventory: LocalItemInventory,
    root_sources: tuple[ReferenceSource, ...],
) -> frozenset[str]:
    """Resolve direct and indirect references to a fixed point.

    Root sources are workflows, Makefile commands, and tests. Each covered
    action manifest or script then becomes another source, except that an
    item's own text cannot cover that item.

    Parameters
    ----------
    inventory : LocalItemInventory
        Local actions and scripts to resolve.
    root_sources : tuple[ReferenceSource, ...]
        Workflow, Makefile and test source text.

    Returns
    -------
    frozenset[str]
        Paths reachable from the root sources through local references.
    """
    covered = {
        item for source in root_sources for item in referenced_items(source, inventory)
    }
    expanded: set[str] = set()
    while pending := covered - expanded:
        for item in sorted(pending):
            expanded.add(item)
            for source in _item_source_texts(item, inventory):
                indirect = referenced_items(source, inventory, source_item=item)
                covered.update(indirect)
    return frozenset(covered)


def uncovered_items(
    inventory: LocalItemInventory,
    covered: frozenset[str],
    exemptions: dict[str, str],
) -> frozenset[str]:
    """Return inventoried paths lacking both a reference and an exemption.

    Parameters
    ----------
    inventory : LocalItemInventory
        Local actions and scripts to check.
    covered : frozenset[str]
        Paths reachable from the root sources.
    exemptions : dict[str, str]
        Paths intentionally excluded, with their reasons.

    Returns
    -------
    frozenset[str]
        Paths with neither a reference nor an exemption.
    """
    return inventory.paths - frozenset(covered) - exemptions.keys()


def exemption_issues(
    inventory: LocalItemInventory,
    covered: frozenset[str],
    exemptions: dict[str, str],
) -> tuple[str, ...]:
    """Describe blank, unknown or stale path-to-reason exemptions.

    Parameters
    ----------
    inventory : LocalItemInventory
        Local actions and scripts that exemptions may name.
    covered : frozenset[str]
        Paths already reachable from the root sources.
    exemptions : dict[str, str]
        Paths and reasons to validate.

    Returns
    -------
    tuple[str, ...]
        Descriptions of blank, unknown or stale exemptions.
    """
    issues: list[str] = []
    blank = sorted(path for path, reason in exemptions.items() if not reason.strip())
    unknown = sorted(exemptions.keys() - inventory.paths)
    stale = sorted(exemptions.keys() & frozenset(covered))
    if blank:
        issues.append(f"exemptions need non-blank reasons: {blank!r}")
    if unknown:
        issues.append(f"exemptions name paths outside the inventory: {unknown!r}")
    if stale:
        issues.append(f"exemptions are stale because items are covered: {stale!r}")
    return tuple(issues)


def _action_is_referenced(text: str, action_path: str) -> bool:
    """Match a local action path without accepting a longer action name."""
    reference = re.escape(f"./{action_path}")
    return re.search(rf"(?<![\w.-]){reference}(?![\w./\\-])", text) is not None


def _script_is_referenced(source: ReferenceSource, script_path: str) -> bool:
    """Match a filename, path or heuristic Python import form.

    Import matching is textual and may treat an import-looking example inside
    a Python fixture string as coverage, hiding an otherwise orphaned script.

    Returns
    -------
    bool
        Whether the source references the inventoried script.
    """
    if source.working_directory is not None and relative_script_is_referenced(
        source, script_path
    ):
        return True
    text = source.text
    if _script_path_is_referenced(text, script_path, source.working_directory):
        return True
    return _python_module_is_imported(source, text, script_path)


def _script_path_is_referenced(
    text: str,
    script_path: str,
    working_directory: str | None,
) -> bool:
    """Match a script filename or path in command text."""
    script_name = Path(script_path).name
    escaped_name = re.escape(script_name)
    escaped_path = re.escape(script_path).replace(r"/", r"[/\\]")
    shell_variable = r"(?:\$\([^)]*\)|\$\{[^}]+\}|\$[A-Za-z_]\w*)"
    filename_patterns = [
        rf"(?<![\w.\\-]){shell_variable}[/\\](?:{escaped_path}|{escaped_name})(?![\w.\\-])"
    ]
    if working_directory is None:
        filename_patterns.extend((
            rf"(?<![\w./\\-]){escaped_name}(?![\w.\\-])",
            rf"(?<![\w./\\-])(?:\./)?{escaped_path}(?![\w.\\-])",
        ))
    return any(re.search(pattern, text) is not None for pattern in filename_patterns)


def _python_module_is_imported(
    source: ReferenceSource,
    text: str,
    script_path: str,
) -> bool:
    """Match supported Python imports for an inventoried module."""
    if not script_path.endswith(".py") or source.source_suffix != ".py":
        return False
    module_name = Path(script_path).stem
    module = re.escape(module_name)
    import_pattern = (
        rf"(?m)^[ \t]*(?:"
        rf"import[ \t]+(?:[^,\n]+,[ \t]*)*(?:{module}|scripts\.{module})(?![\w.])"
        rf"|from[ \t]+(?:{module}|scripts\.{module})[ \t]+import\b"
        rf")"
    )
    python_code = re.sub(r"(?m)#.*$", "", text)
    if re.search(import_pattern, python_code) is not None:
        return True
    return scripts_package_imports_module(text, module_name)


def _item_source_texts(
    item_path: str,
    inventory: LocalItemInventory,
) -> tuple[ReferenceSource, ...]:
    """Read an item's invocation text for the indirect-reference pass."""
    if item_path in inventory.action_manifests:
        return tuple(
            source
            for manifest in inventory.action_manifests[item_path]
            for source in workflow_reference_sources(read_workflow_document(manifest))
        )
    return (
        ReferenceSource(
            read_repository_file(inventory.scripts[item_path]),
            script_suffix=Path(item_path).suffix,
            source_suffix=Path(item_path).suffix,
        ),
    )
