"""Hold every CI helper script to the shared dependency pins and entry shape."""

import re
import typing as typ

import pytest
from ci_script_support import CI_SCRIPT_DIRECTORY

if typ.TYPE_CHECKING:
    import pathlib

REPO_ROOT = CI_SCRIPT_DIRECTORY.parents[1]
MAKEFILE = REPO_ROOT / "Makefile"
SCRIPTS = sorted(
    path for path in CI_SCRIPT_DIRECTORY.glob("*.py") if path.name != "ci_support.py"
)


def _makefile_variable(name: str) -> str:
    text = MAKEFILE.read_text(encoding="utf-8")
    matches = re.findall(rf"^{re.escape(name)} \?= (\S+)$", text, flags=re.MULTILINE)
    assert len(matches) == 1, f"the Makefile must declare {name} exactly once"
    return matches[0]


def test_there_are_scripts_to_hold() -> None:
    """The glob is live; an empty directory would pass the other tests vacuously."""
    assert [path.name for path in SCRIPTS] == [
        "discard_instrumented_tree.py",
        "install_actionlint.py",
        "install_kani.py",
        "report_sccache_stats.py",
        "stage_test_shell.py",
    ], "expected: [path.name for path in SCRIPTS] == [ 'discard_instrumente..."


@pytest.mark.parametrize("path", SCRIPTS, ids=[path.name for path in SCRIPTS])
def test_script_pins_the_makefile_dependency_versions(path: pathlib.Path) -> None:
    """Each PEP 723 block pins cyclopts and cuprum at the Makefile's versions."""
    text = path.read_text(encoding="utf-8")
    cyclopts = _makefile_variable("CYCLOPTS_VERSION")
    cuprum = _makefile_variable("CUPRUM_VERSION")
    header, _, _ = text.partition('# ///\n"""')
    assert header.startswith("#!/usr/bin/env -S uv run --script\n# /// script\n"), (
        path.name
    )
    assert 'requires-python = ">=3.14"' in header, path.name
    assert f'"cyclopts=={cyclopts}"' in header, (
        f"{path.name} must pin cyclopts=={cyclopts}, the Makefile CYCLOPTS_VERSION"
    )
    assert f'"cuprum=={cuprum}"' in header, (
        f"{path.name} must pin cuprum=={cuprum}, the Makefile CUPRUM_VERSION"
    )


@pytest.mark.parametrize("path", SCRIPTS, ids=[path.name for path in SCRIPTS])
def test_script_reads_inputs_from_the_environment_through_cyclopts(
    path: pathlib.Path,
) -> None:
    """Every script is an env-first Cyclopts app with a single default command."""
    text = path.read_text(encoding="utf-8")
    assert 'App(config=cyclopts.config.Env("INPUT_", command=False))' in text, path.name
    assert "@app.default" in text, path.name
    assert text.rstrip().endswith('if __name__ == "__main__":\n    app()'), path.name
