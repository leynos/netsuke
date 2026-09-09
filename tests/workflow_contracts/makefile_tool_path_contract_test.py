"""Contract tests for the Makefile's curated tool PATH.

The Makefile prepends its own user tool directories to ``PATH`` so a caller's
shell need not carry them. These checks hold that curation together with the
resolution of the Go-installed ``actionlint`` workflow linter, so a contributor
who installs it with ``go install`` reaches it without editing ``PATH``.

Run via ``make test-workflow-contracts``.
"""

# This test's contract is the controlled Make invocation, not a linter's
# implementation.
# ruff: ignore[suspicious-subprocess-import] - the boundary is under test.
import subprocess
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: A caller PATH carrying only the system directories: no ``~/.cargo/bin``, no
#: ``~/.bun/bin``, and no Go tool directory. The Makefile's curation is the only
#: way the linters below can be reached.
MINIMAL_PATH = "/usr/bin:/bin"
#: The probe target is defined on the make command line so the repository's
#: Makefile gains no test-only target.
PATH_PROBE_EVAL = 'print-path:;@echo "$$PATH"'
#: Every Go environment shape the curated directory must follow.
GO_TOOL_DIRECTORY_CASES = [
    ({}, "{home}/go/bin"),
    ({"GOBIN": "/opt/netsuke-gobin"}, "/opt/netsuke-gobin"),
    ({"GOPATH": "/opt/netsuke-gopath"}, "/opt/netsuke-gopath/bin"),
]
GO_TOOL_DIRECTORY_IDS = ["home-default", "gobin-override", "gopath-override"]


def _write_stub(path: Path, marker: Path) -> Path:
    """Write an executable stub that records each invocation in ``marker``."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f'#!/bin/sh\nprintf "invoked\\n" >> "{marker}"\n', encoding="utf-8")
    path.chmod(0o755)
    return path


def _make_environment(home: Path, **overrides: str) -> dict[str, str]:
    """Return a minimal environment for a controlled Make invocation."""
    return {"PATH": MINIMAL_PATH, "HOME": str(home), **overrides}


def _run_make(
    home: Path, *arguments: str, **overrides: str
) -> subprocess.CompletedProcess[str]:
    """Run a Makefile target against a minimal, controlled environment."""
    # The command and its arguments are fixed test values; no untrusted input
    # reaches the child process.
    # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
    return subprocess.run(
        [  # ruff: ignore[start-process-with-partial-path] - resolved from the system PATH.
            "make",
            *arguments,
        ],
        check=False,
        cwd=REPO_ROOT,
        env=_make_environment(home, **overrides),
        text=True,
        capture_output=True,
    )


def _observed_path(home: Path, **overrides: str) -> list[str]:
    """Return the PATH a Makefile recipe observes, in order."""
    result = _run_make(home, f"--eval={PATH_PROBE_EVAL}", "print-path", **overrides)

    assert result.returncode == 0, (
        f"the Makefile PATH probe must run; stderr was: {result.stderr}"
    )
    return result.stdout.strip().split(":")


@pytest.mark.parametrize(
    ("overrides", "expected"),
    GO_TOOL_DIRECTORY_CASES,
    ids=GO_TOOL_DIRECTORY_IDS,
)
def test_makefile_curates_the_go_tool_directory_on_path(
    tmp_path: Path, overrides: dict[str, str], expected: str
) -> None:
    """The curated PATH carries the Go tool directory for every Go environment."""
    home = tmp_path / "home"
    curated = _observed_path(home, **overrides)
    go_directory = expected.format(home=home)

    assert go_directory in curated, (
        f"the Makefile must curate the Go tool directory {go_directory} on PATH; "
        f"observed PATH was: {curated}"
    )
    assert curated.index(go_directory) < curated.index("/usr/bin"), (
        "the Go tool directory must precede the caller's inherited PATH"
    )


def test_github_actions_lint_finds_a_go_installed_actionlint(tmp_path: Path) -> None:
    """A minimal caller PATH still reaches an actionlint installed by ``go install``."""
    home = tmp_path / "home"
    actionlint_marker = tmp_path / "actionlint-invoked"
    yamllint_marker = tmp_path / "yamllint-invoked"
    _write_stub(home / "go" / "bin" / "actionlint", actionlint_marker)
    yamllint = _write_stub(tmp_path / "yamllint", yamllint_marker)

    result = _run_make(home, f"YAMLLINT={yamllint}", "github-actions-lint")

    assert result.returncode == 0, (
        "the Makefile must resolve an actionlint installed in the Go tool "
        f"directory without the caller editing PATH; stderr was: {result.stderr}"
    )
    assert actionlint_marker.exists(), (
        "the Makefile must run the actionlint found in the Go tool directory"
    )
    assert yamllint_marker.exists(), "the target must still run the first linter"


def test_missing_actionlint_reports_the_go_tool_directory(tmp_path: Path) -> None:
    """A missing actionlint names the directory it was expected in."""
    home = tmp_path / "home"
    yamllint = _write_stub(tmp_path / "yamllint", tmp_path / "yamllint-invoked")
    expected_path = home / "go" / "bin" / "actionlint"

    result = _run_make(home, f"YAMLLINT={yamllint}", "github-actions-lint")

    assert result.returncode == 2, (
        f"the target must fail when actionlint is absent; stderr was: {result.stderr}"
    )
    assert str(expected_path) in result.stderr, (
        "the failure must name the expected actionlint path rather than a bare "
        f"exit 127; stderr was: {result.stderr}"
    )
