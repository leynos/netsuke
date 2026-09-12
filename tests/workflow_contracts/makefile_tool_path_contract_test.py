"""Contract tests for the Makefile's curated tool PATH.

The Makefile prepends its own user tool directories to ``PATH`` so a caller's
shell need not carry them, and resolves a tool in the recipe shell that
receives that ``PATH`` rather than while Make parses the file. These checks
hold the curation together with the resolution of the Go-installed
``actionlint`` workflow linter, so a contributor who installs it with
``go install`` reaches it without editing ``PATH``.

Run via ``make test-workflow-contracts``.
"""

# This test's contract is the controlled Make invocation, not a linter's
# implementation.
import collections.abc as cabc

# ruff: ignore[suspicious-subprocess-import] - the boundary is under test.
import subprocess
import typing as typ

import pytest
from workflow_loading import MAKEFILE_PATH, REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: A caller PATH carrying only the system directories: no ``~/.cargo/bin``, no
#: ``~/.bun/bin``, and no Go tool directory. The Makefile's curation is the only
#: way the linters below can be reached.
MINIMAL_PATH = "/usr/bin:/bin"
#: The probe target is defined on the make command line so the repository's
#: Makefile gains no test-only target.
PATH_PROBE_EVAL = 'print-path:;@echo "$$PATH"'


def _home_default_directory(sandbox: Path) -> tuple[dict[str, str], Path]:
    """Return the ``$HOME/go/bin`` shape, with no Go variable set."""
    return {}, sandbox / "home" / "go" / "bin"


def _gobin_directory(sandbox: Path) -> tuple[dict[str, str], Path]:
    """Return the ``$GOBIN`` shape, which wins when it is set."""
    directory = sandbox / "gobin"
    return {"GOBIN": str(directory)}, directory


def _gopath_directory(sandbox: Path) -> tuple[dict[str, str], Path]:
    """Return the ``$GOPATH/bin`` shape, which applies when only that is set."""
    gopath = sandbox / "gopath"
    return {"GOPATH": str(gopath)}, gopath / "bin"


#: A Go environment shape: from the sandbox root, the ``GOBIN``/``GOPATH``
#: environment a Make invocation needs, and the tool directory it must reach.
type GoEnvironment = cabc.Callable[[Path], tuple[dict[str, str], Path]]

#: Every Go environment shape the curated directory and the diagnostic follow.
GO_TOOL_DIRECTORY_CASES = [
    pytest.param(_home_default_directory, id="home-default"),
    pytest.param(_gobin_directory, id="gobin-override"),
    pytest.param(_gopath_directory, id="gopath-override"),
]


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


def _run_github_actions_lint(
    sandbox: Path, overrides: dict[str, str]
) -> tuple[subprocess.CompletedProcess[str], Path]:
    """Run the workflow lint target with a stubbed yamllint.

    Returns
    -------
    tuple[subprocess.CompletedProcess[str], Path]
        The completed Make invocation, paired with the marker that records
        whether the first linter ran.
    """
    yamllint_marker = sandbox / "yamllint-invoked"
    yamllint = _write_stub(sandbox / "yamllint", yamllint_marker)

    result = _run_make(
        sandbox / "home", f"YAMLLINT={yamllint}", "github-actions-lint", **overrides
    )
    return result, yamllint_marker


@pytest.mark.parametrize("go_environment", GO_TOOL_DIRECTORY_CASES)
def test_makefile_curates_the_go_tool_directory_on_path(
    tmp_path: Path, go_environment: GoEnvironment
) -> None:
    """The curated PATH carries the Go tool directory for every Go environment."""
    overrides, go_directory = go_environment(tmp_path)

    curated = _observed_path(tmp_path / "home", **overrides)

    assert str(go_directory) in curated, (
        f"the Makefile must curate the Go tool directory {go_directory} on PATH; "
        f"observed PATH was: {curated}"
    )
    assert curated.index(str(go_directory)) < curated.index("/usr/bin"), (
        "the Go tool directory must precede the caller's inherited PATH"
    )


@pytest.mark.parametrize("go_environment", GO_TOOL_DIRECTORY_CASES)
def test_github_actions_lint_finds_a_go_installed_actionlint(
    tmp_path: Path, go_environment: GoEnvironment
) -> None:
    """A minimal caller PATH still reaches an actionlint installed by ``go install``."""
    overrides, go_directory = go_environment(tmp_path)
    actionlint_marker = tmp_path / "actionlint-invoked"
    _write_stub(go_directory / "actionlint", actionlint_marker)

    result, yamllint_marker = _run_github_actions_lint(tmp_path, overrides)

    assert result.returncode == 0, (
        "the Makefile must resolve an actionlint installed in the Go tool "
        f"directory without the caller editing PATH; stderr was: {result.stderr}"
    )
    assert actionlint_marker.exists(), (
        "the Makefile must run the actionlint found in the Go tool directory"
    )
    assert yamllint_marker.exists(), "the target must still run the first linter"


@pytest.mark.parametrize("go_environment", GO_TOOL_DIRECTORY_CASES)
def test_missing_actionlint_reports_the_go_tool_directory(
    tmp_path: Path, go_environment: GoEnvironment
) -> None:
    """A missing actionlint names the directory it was expected in."""
    overrides, go_directory = go_environment(tmp_path)

    result, yamllint_marker = _run_github_actions_lint(tmp_path, overrides)
    expected_path = go_directory / "actionlint"

    assert result.returncode != 0, (
        f"the target must fail when actionlint is absent; stderr was: {result.stderr}"
    )
    assert yamllint_marker.exists(), (
        f"the preflight must not pre-empt the first linter; stderr was: {result.stderr}"
    )
    assert str(expected_path) in result.stderr, (
        "the failure must name the expected actionlint path rather than a bare "
        f"exit 127; stderr was: {result.stderr}"
    )
    assert "ACTIONLINT is" in result.stderr, (
        "the failure must name the configured ACTIONLINT value; stderr was: "
        f"{result.stderr}"
    )


def test_github_actions_lint_honours_an_actionlint_override(tmp_path: Path) -> None:
    """An absolute ACTIONLINT override names the binary to run, as CI does."""
    actionlint_marker = tmp_path / "actionlint-invoked"
    actionlint = _write_stub(tmp_path / "actionlint", actionlint_marker)
    yamllint = _write_stub(tmp_path / "yamllint", tmp_path / "yamllint-invoked")

    result = _run_make(
        tmp_path / "home",
        f"YAMLLINT={yamllint}",
        f"ACTIONLINT={actionlint}",
        "github-actions-lint",
    )

    assert result.returncode == 0, (
        f"the ACTIONLINT override must be honoured; stderr was: {result.stderr}"
    )
    assert actionlint_marker.exists(), "the target must run the overridden actionlint"


def test_default_tool_variables_avoid_parse_time_lookups() -> None:
    """No default tool variable resolves an executable while Make parses the file."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")

    assert "$(shell command -v" not in makefile, (
        "tool resolution belongs in the recipe shell, which is the only shell "
        "that receives the curated PATH; a parse-time `command -v` probe reads "
        "the environment Make was started with, and Make does not reliably "
        "export the fallback variable to that probe"
    )
    for variable, command in (
        ("CARGO", "cargo"),
        ("MDLINT", "markdownlint-cli2"),
        ("ACTIONLINT", "actionlint"),
    ):
        assert f"{variable} ?= {command}\n" in makefile, (
            f"the {variable} default must be the bare command name `{command}` "
            "so the recipe shell resolves it against the curated PATH"
        )
