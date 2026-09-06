"""Pin release-admission runtime tests into the Make and CI contracts.

The Bash gate's subprocess tests validate runtime behaviour and need a
dedicated CI entry point. Workflow-contract tests remain separate because
they validate workflow structure rather than external-command behaviour.

Run via ``make test-workflow-contracts``.
"""

import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - Make is the contract boundary.

from workflow_loading import (
    CI_WORKFLOW_PATH,
    MAKEFILE_PATH,
    job_steps,
    load_workflow,
    named_step,
)

TARGET = "test-release-admission"
RUNTIME_MODULES = (
    "scripts/tests/test_release_admission_metrics.py",
    "scripts/tests/test_release_admission_metric_failures.py",
    "scripts/tests/test_release_admission_metric_boundedness.py",
)
REQUIRED_RECIPE_FRAGMENTS = (
    "PYTHONPATH=scripts",
    "--python $(PYTHON_BASELINE)",
    "--with pytest==9.0.2",
    "--with hypothesis==6.151.9",
    "python -m pytest",
    "-c /dev/null",
    "--rootdir=.",
    "-p no:cacheprovider",
)
REQUIRED_EXPANDED_COMMAND_FRAGMENTS = (
    "PYTHONPATH=scripts",
    "--python 3.14",
    "--with pytest==9.0.2",
    "--with hypothesis==6.151.9",
    "python -m pytest",
    "-c /dev/null",
    "--rootdir=.",
    "-p no:cacheprovider",
)


def _target_recipe() -> str:
    """Return the exact recipe owned by the release-admission Make target.

    Returns
    -------
    str
        The target's tab-indented recipe lines joined with newlines.

    """
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{TARGET}:")),
        None,
    )
    assert start is not None, f"Makefile must define the {TARGET!r} target"
    recipe: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("\t"):
            recipe.append(line.removeprefix("\t"))
        elif line and not line.startswith("#"):
            break
    return "\n".join(recipe)


def _make_executable() -> str:
    """Return the resolved Make executable required for the dry-run boundary.

    Returns
    -------
    str
        Absolute path to the ``make`` executable.

    """
    executable = shutil.which("make")
    assert executable is not None, "the workflow-contract environment must provide make"
    return executable


def test_make_target_runs_only_the_release_admission_runtime_modules() -> None:
    """Require the target to pin each runtime module and execution shape.

    Notes
    -----
    The target must use the repository Python baseline, explicit test
    dependencies, isolated pytest configuration, and exactly three modules.
    """
    recipe = _target_recipe()
    for fragment in (*REQUIRED_RECIPE_FRAGMENTS, *RUNTIME_MODULES):
        assert fragment in recipe, (
            f"{TARGET} must retain {fragment!r} in its pinned runtime command"
        )
    runtime_paths = [
        token for token in recipe.split() if token.startswith("scripts/tests/test_")
    ]
    assert runtime_paths == list(RUNTIME_MODULES), (
        f"{TARGET} must execute exactly the three release-admission modules, "
        f"got {runtime_paths!r}"
    )


def test_make_target_dry_run_has_a_valid_runtime_command() -> None:
    """Require Make to expand the target into the expected command.

    Notes
    -----
    The dry-run contract verifies the executable command without running the
    subprocess test suite.
    """
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the command is fixed and shell is disabled.
        [_make_executable(), "--no-print-directory", "--dry-run", TARGET],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    for fragment in (*REQUIRED_EXPANDED_COMMAND_FRAGMENTS, *RUNTIME_MODULES):
        assert fragment in result.stdout, (
            f"the expanded {TARGET} command must retain {fragment!r}"
        )


def test_pull_request_ci_runs_runtime_tests_separately_from_workflow_contracts() -> (
    None
):
    """Require CI to separate runtime and workflow-contract validation.

    Notes
    -----
    The workflow-contract target remains an independent gate and precedes the
    dedicated release-admission runtime target.
    """
    steps = job_steps(load_workflow(CI_WORKFLOW_PATH), "build-test")
    workflow_contracts = named_step(steps, "Workflow contract tests")
    runtime = named_step(steps, "Release-admission runtime tests")
    assert workflow_contracts.get("run") == "make test-workflow-contracts", (
        "CI must retain the separate workflow-contract target"
    )
    assert runtime.get("run") == f"make {TARGET}", (
        "CI must invoke the dedicated release-admission runtime target"
    )
    assert steps.index(workflow_contracts) < steps.index(runtime), (
        "CI must run the runtime tests as their own gate after workflow contracts"
    )
