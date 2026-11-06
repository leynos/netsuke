"""The docstring gate fails on a single missing docstring in a large tree.

Interrogate rounds its coverage percentage to one decimal before it compares
that figure with ``--fail-under``, so a lone undocumented definition among a few
thousand passes a bare threshold. ``make interrogate`` therefore prints every
definition and fails on any ``MISSED`` row. These tests run the real pinned
tool, through the real target, over generated trees: one fully documented, and
one with a single gap that the bare threshold does not see.

Run via ``make test-workflow-contracts``.
"""

import os
import shutil

# This contract runs `make interrogate`, which runs the pinned tool, as a child.
# ruff: ignore[suspicious-subprocess-import] - the gate itself is under test.
import subprocess
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: Enough definitions that one gap rounds away: 1 in 4,001 is 99.975%.
DEFINITION_COUNT = 4000


def _write_tree(root: Path, *, undocumented: int) -> Path:
    """Write a package of ``DEFINITION_COUNT`` documented functions, less a few."""
    package = root / "generated"
    package.mkdir()
    lines = ['"""A generated module."""', ""]
    for index in range(DEFINITION_COUNT):
        body = ["    return None", ""]
        if index < DEFINITION_COUNT - undocumented:
            body.insert(0, '    """Do nothing."""')
        lines.extend([f"def documented_{index}() -> None:", *body])
    init = package / "__init__.py"
    init.write_text('"""A generated package."""\n', encoding="utf-8")
    (package / "module.py").write_text("\n".join(lines), encoding="utf-8")
    return package


def _make_interrogate(
    package: Path, *, flags: str | None = None
) -> subprocess.CompletedProcess[str]:
    """Run the real ``make interrogate`` over ``package``."""
    make = shutil.which("make")
    assert make, "make must be on the path"
    arguments = [
        make,
        "-C",
        str(REPO_ROOT),
        "interrogate",
        f"INTERROGATE_SOURCES={package}",
    ]
    if flags is not None:
        arguments.append(f"INTERROGATE_FLAGS={flags}")
    # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
    return subprocess.run(
        arguments,
        check=False,
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        env=os.environ.copy(),
    )


@pytest.fixture(scope="module")
def gapped_package(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Build a large tree with one undocumented function."""
    return _write_tree(tmp_path_factory.mktemp("gapped"), undocumented=1)


def test_the_target_passes_a_fully_documented_tree(tmp_path: Path) -> None:
    """With no gap, the real gate succeeds and prints the report."""
    result = _make_interrogate(_write_tree(tmp_path, undocumented=0))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "RESULT: PASSED" in result.stdout, "the report must be printed"


def test_a_bare_threshold_does_not_see_the_gap(gapped_package: Path) -> None:
    """The fixture is the rounding case: the percentage alone passes it.

    Without this, the next test would prove nothing about the guard.
    """
    result = _make_interrogate(gapped_package, flags="--fail-under 100")
    assert result.returncode == 0, (
        "the generated gap must be hidden by the rounded percentage; "
        f"stdout was: {result.stdout}"
    )


def test_the_target_fails_on_the_single_missing_docstring(
    gapped_package: Path,
) -> None:
    """The MISSED guard fails the target where the bare threshold passes."""
    result = _make_interrogate(gapped_package)
    assert result.returncode != 0, "a single missing docstring must fail the gate"
    assert "undocumented definitions remain" in result.stderr, result.stderr
    assert "MISSED" in result.stdout, "the report must name the missed definition"
