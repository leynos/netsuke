"""Contract tests for the Pylint lint tier.

The tier runs a pinned Pylint as a uv tool on managed CPython at the Python
baseline (`$(PYTHON_BASELINE)`). Two properties of that tier are easy to lose
without any test noticing: the interpreter pin, which decides the grammar
Pylint parses with, and the `syntax-error` message, which decides whether a
module that grammar cannot parse fails the lint or is skipped without a word.
Elsewhere in the estate both regressed silently: an unpinned `pypy` moved to a
newer PyPy with no commit, and a disabled `syntax-error` let unparsable modules
go unlinted.
"""

import re
import shlex
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the end-to-end test drives Make.
import tomllib
from pathlib import Path

import pytest

_REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = _REPO_ROOT / "Makefile"
CONFIG_PATH = _REPO_ROOT / "pyproject.toml"

_PROBE_TARGET: str = "jm5-pylint-probe"


def _makefile_variable(name: str) -> str:
    """Return the value assigned to a Makefile variable.

    Parameters
    ----------
    name : str
        The variable name, assigned with ``=`` or ``?=``.

    Returns
    -------
    str
        The assigned value with surrounding whitespace removed.
    """
    # Join backslash continuations so a multi-line assignment reads whole.
    text = MAKEFILE_PATH.read_text(encoding="utf-8").replace("\\\n", " ")
    match = re.search(rf"^{re.escape(name)}\s*\??=\s*(.+)$", text, flags=re.MULTILINE)
    assert match is not None, f"{name} is not defined in the Makefile"
    return match.group(1).strip()


def test_pylint_tier_pins_the_interpreter() -> None:
    """The tier must name the interpreter whose grammar it parses with."""
    assert _makefile_variable("PYLINT_PYTHON") == "$(PYTHON_BASELINE)", (
        "PYLINT_PYTHON must stay $(PYTHON_BASELINE): Pylint must parse at the baseline"
    )


def test_pylint_tier_runs_the_pinned_release_on_managed_python() -> None:
    """The tier must run the pinned Pylint on a uv-managed interpreter."""
    assert _makefile_variable("PYLINT_VERSION") == "4.0.9", (
        "PYLINT_VERSION must pin the reviewed Pylint release exactly"
    )
    command = _makefile_variable("PYLINT")
    for fragment in (
        "tool run --managed-python --python $(PYLINT_PYTHON)",
        "--from 'pylint==$(PYLINT_VERSION)' pylint",
    ):
        assert fragment in command, f"PYLINT must contain {fragment!r}: {command}"


def test_pylint_reports_unparsable_modules() -> None:
    """The Pylint policy must not disable `syntax-error`."""
    config = tomllib.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    disabled = config["tool"]["pylint"]["messages control"]["disable"]
    assert "syntax-error" not in disabled, (
        "pyproject.toml must not disable syntax-error: a module the interpreter "
        f"cannot parse would then be skipped silently; disable={disabled!r}"
    )


def _make_quoted(path: Path) -> str:
    """Quote a path for a shell word inside a Make recipe.

    Parameters
    ----------
    path : Path
        The path to quote.

    Returns
    -------
    str
        The shell-quoted path with each ``$`` doubled, so Make passes it to
        the shell as one literal word.
    """
    return shlex.quote(str(path)).replace("$", "$$")


def _run_configured_pylint(target: Path) -> subprocess.CompletedProcess[str]:
    """Run the Makefile's own `$(PYLINT)` command over one module.

    Parameters
    ----------
    target : Path
        The module to lint.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished Make process, with output captured.
    """
    make = shutil.which("make")
    assert make is not None, "make must be on PATH"
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argument list, no shell.
        [
            make,
            "--no-print-directory",
            "-s",
            "-C",
            str(_REPO_ROOT),
            "--eval",
            f"{_PROBE_TARGET}: ; $(PYLINT) {_make_quoted(target)}",
            _PROBE_TARGET,
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=600,
    )


@pytest.mark.skipif(
    shutil.which("uv") is None or shutil.which("make") is None,
    reason="the end-to-end check needs make and uv",
)
def test_configured_pylint_fails_on_an_unparsable_module(tmp_path: Path) -> None:
    """The configured tier must fail on a parse error and pass a clean module.

    This drives the real `$(PYLINT)` command, so it catches a disabled
    `syntax-error` wherever it is configured, not only in the file the
    contract above reads.
    """
    broken = tmp_path / "broken_module.py"
    broken.write_text("def broken(\n    return 1\n", encoding="utf-8")
    clean = tmp_path / "clean_module.py"
    clean.write_text('"""A clean module."""\n\nVALUE = 1\n', encoding="utf-8")

    failed = _run_configured_pylint(broken)
    passed = _run_configured_pylint(clean)

    assert failed.returncode != 0, (
        f"a module Pylint cannot parse must fail the tier: {failed.stdout}"
    )
    assert "syntax-error" in failed.stdout, (
        f"the failure must be the parse error: {failed.stdout}{failed.stderr}"
    )
    assert passed.returncode == 0, (
        f"a clean module must pass the tier: {passed.stdout}{passed.stderr}"
    )
