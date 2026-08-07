"""Contract tests for the CI workflow's lint and test-shell prerequisites.

Two invariants in ``.github/workflows/ci.yml`` are load-bearing but easy to
lose to an innocuous edit, and both fail in ways that are slow and confusing
to diagnose from a red run:

* The hermetic dev-fast sandbox probes ``awk`` through a capability-backed
  directory handle, which deliberately cannot follow a symlink that escapes
  its containing directory. Ubuntu ships ``awk`` as exactly such a symlink,
  so the job installs ``gawk`` and copies it to a regular executable on the
  job ``PATH``. Dropping any part of that dance turns into a sandbox probe
  failure far from its cause.
* ``make lint`` must run, and the Makefile's Clippy flags must stay
  workspace-wide. Narrowing them silently stops linting ``test_support``
  and the non-default targets.

These tests parse the workflow with PyYAML and pin that contract, so drift
fails on the pull request rather than in a later run.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
MAKEFILE_PATH = REPO_ROOT / "Makefile"

TEST_SHELL_STEP = "Install test shell dependencies"

#: Clippy must stay workspace-wide with warnings denied; a narrower flag set
#: silently drops `test_support` and the non-default targets from the gate.
EXPECTED_CLIPPY_FLAGS = "--workspace --all-targets --all-features -- -D warnings"


def _load() -> dict[str, object]:
    """Parse the workflow file."""
    return yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _steps(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the build-test job's steps."""
    match workflow.get("jobs"):
        case dict() as jobs:
            pass
        case _:
            message = "the workflow must declare a jobs mapping"
            raise AssertionError(message)
    match jobs.get("build-test"):
        case dict() as job:
            pass
        case _:
            message = "the workflow must declare a build-test job"
            raise AssertionError(message)
    match job.get("steps"):
        case list() as steps:
            return steps
        case _:
            message = "jobs.build-test.steps must be a list"
            raise AssertionError(message)


def _step(name: str) -> dict[str, object]:
    """Return the uniquely named step from the build-test job."""
    matches = [step for step in _steps(_load()) if step.get("name") == name]
    assert len(matches) == 1, (
        f"expected exactly one step named {name!r}, found {len(matches)}"
    )
    return matches[0]


def _test_shell_script() -> str:
    """Return the run script of the test-shell dependency step."""
    match _step(TEST_SHELL_STEP).get("run"):
        case str() as run:
            return run
        case _:
            message = f"{TEST_SHELL_STEP} must declare a run script"
            raise AssertionError(message)


def test_test_shell_step_installs_gawk() -> None:
    """The step installs gawk, the implementation awk is copied from."""
    script = _test_shell_script()
    assert re.search(r"apt-get install\b.*\bgawk\b", script), (
        f"{TEST_SHELL_STEP} must apt-get install gawk, got:\n{script}"
    )


def test_test_shell_step_copies_gawk_to_a_regular_awk_executable() -> None:
    """Gawk is copied — not linked — to ${RUNNER_TEMP}/netsuke-test-bin/awk.

    The sandbox probe cannot follow a symlink out of its directory handle, so
    the destination must be a regular executable file.
    """
    script = _test_shell_script()
    assert 'install --mode=0755 "$(command -v gawk)"' in script, (
        f"{TEST_SHELL_STEP} must copy $(command -v gawk) as a regular "
        f"executable, got:\n{script}"
    )
    assert '"${test_shell_bin}/awk"' in script, (
        f"{TEST_SHELL_STEP} must install the copy as awk, got:\n{script}"
    )
    assert 'test_shell_bin="${RUNNER_TEMP}/netsuke-test-bin"' in script, (
        f"{TEST_SHELL_STEP} must stage the copy in "
        f"${{RUNNER_TEMP}}/netsuke-test-bin, got:\n{script}"
    )


def test_test_shell_step_exports_the_directory_to_github_path() -> None:
    """Later steps see the staged awk because the directory joins PATH."""
    script = _test_shell_script()
    assert 'echo "${test_shell_bin}" >> "${GITHUB_PATH}"' in script, (
        f"{TEST_SHELL_STEP} must append the staging directory to "
        f"GITHUB_PATH, got:\n{script}"
    )


def test_test_shell_step_verifies_the_staged_awk() -> None:
    """The step proves the staged awk resolves and runs before CI proceeds."""
    script = _test_shell_script()
    lines = [line.strip() for line in script.splitlines()]
    assert "command -v awk" in lines, (
        f"{TEST_SHELL_STEP} must run `command -v awk`, got:\n{script}"
    )
    assert "awk --version" in lines, (
        f"{TEST_SHELL_STEP} must run `awk --version`, got:\n{script}"
    )


def test_workflow_runs_make_lint() -> None:
    """CI runs the lint gate through the Makefile, not an ad hoc command."""
    runs = [step.get("run") for step in _steps(_load())]
    assert "make lint" in runs, (
        f"the build-test job must run `make lint`, got run steps: {runs!r}"
    )


def test_makefile_clippy_flags_stay_workspace_wide() -> None:
    """CLIPPY_FLAGS covers the whole workspace with warnings denied."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = re.search(r"^CLIPPY_FLAGS \?= (.+)$", makefile, re.MULTILINE)
    assert match is not None, "the Makefile must define CLIPPY_FLAGS"
    assert match.group(1).strip() == EXPECTED_CLIPPY_FLAGS, (
        f"CLIPPY_FLAGS must be {EXPECTED_CLIPPY_FLAGS!r} so Clippy covers "
        f"every workspace crate, target, and feature with warnings denied; "
        f"got {match.group(1).strip()!r}"
    )
