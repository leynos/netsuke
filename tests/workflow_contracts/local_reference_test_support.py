"""Build minimal repositories for local-reference contract tests.

The workspace writer is shared by the coverage and edge-case suites. It stays
private to those tests so synthetic files exercise the scanner without adding
references to the live repository inventory.
"""

from pathlib import Path


def create_synthetic_workspace(
    root: Path,
    workflow: str,
    makefile: str,
    items: dict[str, str],
) -> None:
    """Write workflow, Makefile and local items into a temporary repository."""
    (root / ".github" / "actions").mkdir(parents=True, exist_ok=True)
    (root / "scripts").mkdir(exist_ok=True)
    write_synthetic_file(root, ".github/workflows/ci.yml", workflow)
    write_synthetic_file(root, "Makefile", makefile)
    for relative_path, contents in items.items():
        write_synthetic_file(root, relative_path, contents)
    (root / "tests").mkdir(exist_ok=True)
    (root / "scripts" / "tests").mkdir(parents=True, exist_ok=True)


def write_synthetic_file(root: Path, relative_path: str, contents: str) -> None:
    """Create one UTF-8 file and its parent directories in a test workspace."""
    path = Path(root, relative_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")
