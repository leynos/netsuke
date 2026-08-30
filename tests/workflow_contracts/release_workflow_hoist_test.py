"""Contract tests wiring the hoist step into ``release.yml``.

The behavioural suites prove what ``scripts/hoist_binstall_archives.py``
does; these tests pin how the release workflow invokes it: with the
resolved version, under a pinned interpreter, without a restored uv cache,
and before the asset upload publishes the hoisted archives.

Run via ``make test-workflow-contracts``.
"""

import re

from workflow_loading import (
    MAKEFILE_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
)


def _python_baseline() -> str:
    """Return the Python baseline the Makefile pins for uv-driven tooling.

    ``python_toolchain_sync_test.py`` holds this value equal to the CI
    workflow env, so reading it here keeps the hoist contract on the single
    source rather than repeating a literal that would drift on the next bump.

    Returns
    -------
    str
        The ``PYTHON_BASELINE ?=`` default declared by the Makefile.
    """
    text = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = re.search(r"^PYTHON_BASELINE \?= (\S+)$", text, flags=re.MULTILINE)
    assert match is not None, "the Makefile must declare PYTHON_BASELINE ?="
    return match.group(1)


def test_release_workflow_invokes_the_hoist_script() -> None:
    """The release job must run the script with the resolved version."""
    workflow = RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
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
    baseline = _python_baseline()
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), "release")
    hoist_index = step_index_by_key(steps, "run", "hoist_binstall_archives.py")
    assert f"--python {baseline}" in str(steps[hoist_index]["run"]), (
        "the hoist step must pin the interpreter version it runs under"
    )
    setup_index = step_index_by_key(steps, "uses", "setup-uv")
    assert setup_index < hoist_index, (
        "the interpreter must be installed before the hoist step runs"
    )
    setup_with = require_mapping(
        steps[setup_index].get("with"), "the setup-uv step's with block"
    )
    assert setup_with["python-version"] == baseline, (
        "the installed interpreter must match the version the hoist step pins"
    )


def test_release_workflow_disables_the_uv_cache() -> None:
    """The privileged release job must not restore a uv cache.

    The hoist script is stdlib-only and runs with ``--no-project``, so a
    restored cache buys nothing while adding a supply-chain input to a job
    holding ``contents: write``.
    """
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), "release")
    setup = steps[step_index_by_key(steps, "uses", "setup-uv")]
    setup_with = require_mapping(setup.get("with"), "the setup-uv step's with block")
    assert setup_with["enable-cache"] is False, (
        "setup-uv in the release job must set enable-cache: false"
    )


def test_release_workflow_hoists_before_uploading() -> None:
    """The hoist step must precede the asset upload in the release job."""
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), "release")
    hoist_index = step_index_by_key(steps, "run", "hoist_binstall_archives.py")
    upload_index = step_index_by_key(steps, "id", "upload_assets")
    assert hoist_index < upload_index, (
        "the hoist must run before upload_assets so only validated, hoisted "
        "archives are published"
    )
