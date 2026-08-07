"""Contract tests wiring the hoist step into ``release.yml``.

The behavioural suites prove what ``scripts/hoist_binstall_archives.py``
does; these tests pin how the release workflow invokes it: with the
resolved version, under a pinned interpreter, without a restored uv cache,
and before the asset upload publishes the hoisted archives.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path

import yaml

WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "release.yml"
)


def test_release_workflow_invokes_the_hoist_script() -> None:
    """The release job must run the script with the resolved version."""
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    assert "scripts/hoist_binstall_archives.py" in workflow, (
        "release.yml must invoke the hoist script"
    )
    assert "--version '${{ needs.metadata.outputs.version }}'" in workflow, (
        "release.yml must pass the resolved release version to the script"
    )


def test_release_workflow_pins_the_hoist_interpreter() -> None:
    """The hoist must run under a pinned Python, not the runner's default.

    The script relies on `BaseExceptionGroup`, which needs Python 3.11 or
    later, so the release job installs the interpreter the repository's Python
    tooling targets rather than trusting whatever `python3` the runner ships.
    """
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    steps = workflow["jobs"]["release"]["steps"]
    hoist_index = next(
        index
        for index, step in enumerate(steps)
        if "hoist_binstall_archives.py" in step.get("run", "")
    )
    assert "--python 3.13" in steps[hoist_index]["run"], (
        "the hoist step must pin the interpreter version it runs under"
    )
    setup_index = next(
        index for index, step in enumerate(steps) if "setup-uv" in step.get("uses", "")
    )
    assert setup_index < hoist_index, (
        "the interpreter must be installed before the hoist step runs"
    )
    assert steps[setup_index]["with"]["python-version"] == "3.13", (
        "the installed interpreter must match the version the hoist step pins"
    )


def test_release_workflow_disables_the_uv_cache() -> None:
    """The privileged release job must not restore a uv cache.

    The hoist script is stdlib-only and runs with ``--no-project``, so a
    restored cache buys nothing while adding a supply-chain input to a job
    holding ``contents: write``.
    """
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    steps = workflow["jobs"]["release"]["steps"]
    setup = next(step for step in steps if "setup-uv" in step.get("uses", ""))
    assert setup["with"]["enable-cache"] is False, (
        "setup-uv in the release job must set enable-cache: false"
    )


def test_release_workflow_hoists_before_uploading() -> None:
    """The hoist step must precede the asset upload in the release job."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    steps = workflow["jobs"]["release"]["steps"]
    hoist_index = next(
        index
        for index, step in enumerate(steps)
        if "hoist_binstall_archives.py" in step.get("run", "")
    )
    upload_index = next(
        index for index, step in enumerate(steps) if step.get("id") == "upload_assets"
    )
    assert hoist_index < upload_index, (
        "the hoist must run before upload_assets so only validated, hoisted "
        "archives are published"
    )
