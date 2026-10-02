"""Protect the package workflow's Python setup and staged-output boundaries.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import PACKAGE_WORKFLOW_PATH, load_workflow, require_mapping


def _build_steps() -> list[dict[str, object]]:
    """Return parsed steps from the reusable build job."""
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    jobs = require_mapping(workflow.get("jobs"), "build-and-package.yml jobs")
    build = require_mapping(jobs.get("build"), "build-and-package.yml build job")
    steps = build.get("steps")
    assert isinstance(steps, list), "build job steps must be a list"
    return [
        require_mapping(step, f"build-and-package.yml build step {index}")
        for index, step in enumerate(steps)
    ]


def test_build_job_installs_uv_for_all_platforms() -> None:
    """Keep setup-uv and its configured interpreter on every build platform."""
    setup_steps = [step for step in _build_steps() if step.get("name") == "Install uv"]
    assert len(setup_steps) == 1, "build job must have one Install uv step"
    setup = setup_steps[0]
    uses = setup.get("uses")
    assert isinstance(uses, str), "Install uv must declare its action"
    assert uses.startswith("astral-sh/setup-uv@"), (
        "Install uv must use the pinned astral-sh/setup-uv action"
    )
    inputs = require_mapping(setup.get("with"), "Install uv inputs")
    assert inputs.get("python-version") == "${{ inputs['python-version'] }}", (
        "Install uv must use the configured workflow Python version"
    )
    assert "if" not in setup, "Install uv must run on every build platform"


def test_build_job_does_not_reintroduce_capture_staged_paths() -> None:
    """Keep staged outputs available directly to later workflow steps."""
    names = [step.get("name") for step in _build_steps()]
    assert "Capture staged paths" not in names, (
        "later steps must consume staged outputs directly"
    )
