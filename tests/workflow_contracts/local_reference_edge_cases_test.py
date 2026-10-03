"""Reject misleading or unreadable local-reference sources.

Run via ``make test-workflow-contracts``.
"""

import shutil
from pathlib import Path

import pytest
from cargo_test_targets import RepositoryFileError
from local_reference_inventory import discover_local_items
from local_reference_test_support import (
    create_synthetic_workspace,
    write_synthetic_file,
)
from local_references import (
    covered_items,
    root_source_texts,
    uncovered_items,
)


def test_filename_near_matches_do_not_cover_scripts(tmp_path: Path) -> None:
    """Reject filename continuations and paths naming another directory."""
    create_synthetic_workspace(
        tmp_path,
        "",
        "all:\n\t@printf '%s' scripts/orphan.sh.bak scripts/orphan.sh-extra "
        "vendor/other_tool.py\n",
        {"scripts/orphan.sh": "", "scripts/other_tool.py": ""},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/orphan.sh",
        "scripts/other_tool.py",
    }), "near-match filenames and other directory paths must not count"


@pytest.mark.parametrize(
    "relative_directory",
    [".github/actions", "scripts"],
    ids=["actions", "scripts"],
)
def test_missing_local_inventory_root_reports_its_path(
    tmp_path: Path,
    relative_directory: str,
) -> None:
    """Fail closed with a path-aware error when an inventory root is missing."""
    create_synthetic_workspace(tmp_path, "", "all:\n\t@true\n", {})
    shutil.rmtree(tmp_path / relative_directory)

    with pytest.raises(RepositoryFileError, match=relative_directory) as error:
        discover_local_items(tmp_path)

    assert isinstance(error.value.__cause__, FileNotFoundError), (
        "the inventory error must preserve the missing-directory cause"
    )


def test_action_child_path_does_not_cover_action(tmp_path: Path) -> None:
    """Require the action directory path without a child-file suffix."""
    action_path = ".github/actions/deploy"
    create_synthetic_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        f"all:\n\t@printf '%s' ./{action_path}/README.md\n",
        {f"{action_path}/action.yml": "name: Deploy\n"},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == frozenset({action_path}), (
        "an action child path must not count as an action reference"
    )


@pytest.mark.parametrize(
    ("working_directory", "expected_uncovered"),
    [
        ("scripts", frozenset()),
        ("vendor", frozenset({"scripts/helper.sh"})),
    ],
    ids=["script-directory", "different-directory"],
)
def test_script_current_directory_invocation_respects_working_directory(
    tmp_path: Path,
    working_directory: str,
    expected_uncovered: frozenset[str],
) -> None:
    """Resolve a relative workflow command from its declared directory."""
    create_synthetic_workspace(
        tmp_path,
        (
            "name: Synthetic\n"
            "jobs:\n"
            "  test:\n"
            "    steps:\n"
            "      - run: ./helper.sh\n"
            f"        working-directory: {working_directory}\n"
        ),
        "all:\n\t@true\n",
        {"scripts/helper.sh": ""},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == expected_uncovered, (
        "a ./filename invocation must be resolved against its working directory"
    )


@pytest.mark.parametrize(
    ("script_suffix", "script_text"),
    [
        (".bash", "printf ready # scripts/orphan.sh\n# scripts/orphan.sh\n"),
        (".py", "print('ready') # scripts/orphan.sh\n# scripts/orphan.sh\n"),
        (
            ".ps1",
            "Write-Output ready # scripts/orphan.sh\n<# scripts/orphan.sh #>\n",
        ),
        (".sh", "printf ready # scripts/orphan.sh\n# scripts/orphan.sh\n"),
    ],
)
def test_comments_in_reached_scripts_do_not_cover_an_orphan(
    tmp_path: Path,
    script_suffix: str,
    script_text: str,
) -> None:
    """Ignore comment-only and trailing links in reached script source."""
    launcher = f"scripts/launcher{script_suffix}"
    create_synthetic_workspace(
        tmp_path,
        "name: Synthetic\njobs: {}\n",
        f"all:\n\t./{launcher}\n",
        {launcher: script_text, "scripts/orphan.sh": ""},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert covered == {launcher}, "script comments must not extend reachability"
    assert "scripts/orphan.sh" in uncovered_items(inventory, covered, {}), (
        "a link found only in a script comment must leave the target uncovered"
    )


def test_import_aliases_and_comments_do_not_cover_modules(tmp_path: Path) -> None:
    """Count only the module imported, not aliases or comments."""
    create_synthetic_workspace(
        tmp_path,
        "",
        "all:\n\t@true\n",
        {
            "scripts/alias_target.py": "",
            "scripts/comment_target.py": "",
            "scripts/ordinary_comment_target.py": "",
        },
    )
    write_synthetic_file(
        tmp_path,
        "tests/test_import_aliases.py",
        "from scripts import different_module as alias_target  # comment_target\n"
        "import different_module  # comment, ordinary_comment_target\n",
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/alias_target.py",
        "scripts/comment_target.py",
        "scripts/ordinary_comment_target.py",
    }), "aliases and trailing comments must not establish module coverage"


def test_windows_foreign_directory_does_not_cover_script(tmp_path: Path) -> None:
    """Treat backslashes as separators before inventoried script names."""
    create_synthetic_workspace(
        tmp_path,
        "",
        "all:\n\t@printf '%s' vendor\\other_tool.py\n",
        {"scripts/other_tool.py": ""},
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/other_tool.py"
    }), "a script basename after a foreign Windows-style directory must not count"


def test_binary_test_fixtures_are_skipped(tmp_path: Path) -> None:
    """Skip unsupported or undecodable fixtures while scanning test sources."""
    create_synthetic_workspace(
        tmp_path,
        "",
        "all:\n\t@true\n",
        {"scripts/orphan.sh": ""},
    )
    binary_fixture = Path(tmp_path, "tests", "fixtures", "image.bin")
    binary_fixture.parent.mkdir()
    binary_fixture.write_bytes(b"\xffscripts/orphan.sh")
    encoded_fixture = Path(tmp_path, "tests", "fixtures", "encoded.txt")
    encoded_fixture.write_bytes(b"\xffscripts/orphan.sh")
    inventory = discover_local_items(tmp_path)

    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert not covered, "binary fixtures must not count as reference sources"
    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/orphan.sh"
    }), "references inside binary fixtures must not cover an orphan script"


def test_import_looking_text_fixture_does_not_cover_python_script(
    tmp_path: Path,
) -> None:
    """Restrict heuristic module-import matching to Python source files."""
    create_synthetic_workspace(
        tmp_path,
        "",
        "all:\n\t@true\n",
        {"scripts/fixture_target.py": ""},
    )
    write_synthetic_file(
        tmp_path,
        "tests/fixtures/import-example.txt",
        "import fixture_target\n",
    )
    inventory = discover_local_items(tmp_path)
    covered = covered_items(inventory, root_source_texts(tmp_path))

    assert uncovered_items(inventory, covered, {}) == frozenset({
        "scripts/fixture_target.py"
    }), "text fixtures must not count as Python module imports"


@pytest.mark.parametrize("relative_directory", ["tests", "scripts/tests"])
def test_missing_test_source_root_fails_closed(
    tmp_path: Path,
    relative_directory: str,
) -> None:
    """Reject a scan that silently omits one of its required test roots."""
    create_synthetic_workspace(tmp_path, "", "all:\n\t@true\n", {})
    missing_directory = tmp_path / relative_directory
    missing_directory.rmdir()

    with pytest.raises(RepositoryFileError, match=relative_directory):
        root_source_texts(tmp_path)


def test_invalid_utf8_script_reports_its_path_and_decode_error(
    tmp_path: Path,
) -> None:
    """Name scripts that cannot be decoded during indirect scanning."""
    script_path = "scripts/invalid_source.py"
    create_synthetic_workspace(
        tmp_path,
        "",
        f"all:\n\tpython {script_path}\n",
        {script_path: ""},
    )
    (tmp_path / script_path).write_bytes(b"\xff")
    inventory = discover_local_items(tmp_path)

    with pytest.raises(RepositoryFileError, match=script_path) as error:
        covered_items(inventory, root_source_texts(tmp_path))

    assert isinstance(error.value.__cause__, UnicodeDecodeError), (
        "the path-specific error must preserve the decoding failure as its cause"
    )


def test_invalid_utf8_makefile_reports_its_path_and_decode_error(
    tmp_path: Path,
) -> None:
    """Name a Makefile that cannot be decoded during root-source scanning."""
    create_synthetic_workspace(tmp_path, "", "all:\n\t@true\n", {})
    (tmp_path / "Makefile").write_bytes(b"\xff")

    with pytest.raises(RepositoryFileError, match="Makefile") as error:
        root_source_texts(tmp_path)

    assert isinstance(error.value.__cause__, UnicodeDecodeError), (
        "the path-specific error must preserve the decoding failure as its cause"
    )
