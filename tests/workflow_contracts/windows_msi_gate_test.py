"""Contract tests for the Windows MSI upgrade gate.

The `windows-msi-upgrade` job is the one lane that can catch what neither XML
parsing nor compilation can: authoring WiX has stopped supporting, and Windows
Installer's own replacement behaviour. It delegates both to the repository's
`windows-msi-upgrade-validation` action, which builds a sequence of MSI
fixtures with the same custom WXS the release ships and drives the transitions
between them.

Each of those is a separate way the gate can quietly stop being a gate. An
unpinned packaging action runs someone else's code; a fixture built with
different authoring, or a different packaging revision from the release lane's,
tests a package the release never produces; a fixture whose version drifted
leaves an upgrade path silently untested; and a missing script leaves the gate
unable to report or the runner dirty for the next job.

These live here rather than in ``ci_windows_job_test.py`` to keep both modules
inside the repository's 400-line file limit, following ``ci_windows_lint_test``
and ``ci_windows_smoke_test``. Shared parsing helpers live in
``workflow_loading.py``; the action-reference helper lives in
``action_references.py``.

Run via ``make test-workflow-contracts``.
"""

import pytest
from action_references import require_external_action_sha
from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    MSI_VALIDATION_ACTION_PATH,
    PACKAGE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    read_workflow_document,
    require_list,
    require_mapping,
)

MSI_JOB = "windows-msi-upgrade"

#: The repository-owned composite action that builds the MSI fixtures and
#: exercises the upgrade paths.
MSI_VALIDATION_ACTION = "./.github/actions/windows-msi-upgrade-validation"
#: The shared action that builds a Windows installer package. The MSI fixture
#: lanes and the release packaging lane must exercise the same one.
WINDOWS_PACKAGE_ACTION = "leynos/shared-actions/.github/actions/windows-package"
#: The three ordered upgrade fixtures, each built by its own packaging step.
MSI_FIXTURE_STEPS = (
    "Build beta1 MSI fixture",
    "Build beta2 MSI fixture",
    "Build final MSI fixture",
)
#: Each fixture's version. The order is the upgrade sequence — beta1 before
#: beta2 before final — so a fixture that reused another's version would leave
#: a transition untested while the suite still passed.
MSI_FIXTURE_VERSIONS = {
    "Build beta1 MSI fixture": "1.2.3-beta1",
    "Build beta2 MSI fixture": "1.2.3-beta2",
    "Build final MSI fixture": "1.2.3",
}
#: Both repository-owned scripts the gate drives: the first runs the
#: transitions, the second removes whatever the run installed.
MSI_VALIDATION_SCRIPTS = (
    "windows-msi-upgrade-validation.ps1",
    "windows-msi-upgrade-cleanup.ps1",
)


def _windows_package_sha(step: dict[str, object], description: str) -> str:
    """Return the pin a step uses for the shared Windows packaging action."""
    return require_external_action_sha(
        step.get("uses"), WINDOWS_PACKAGE_ACTION, description
    )


def _msi_fixture_steps() -> dict[str, dict[str, object]]:
    """Return the MSI validation action's three packaging steps, by name.

    The action is parsed as YAML rather than scanned as text: a text scan
    cannot tell a step's ``uses`` from the same string sitting in a comment or
    an input, and it cannot count steps at all. The three fixtures are an
    ordered upgrade sequence, so their count is part of the contract.

    Returns
    -------
    dict[str, dict[str, object]]
        Each fixture step name mapped to its parsed step mapping.
    """
    document = require_mapping(
        read_workflow_document(MSI_VALIDATION_ACTION_PATH),
        "the MSI upgrade validation action",
    )
    runs = require_mapping(document.get("runs"), "the action's runs block")
    steps = [
        require_mapping(step, f"action step {index}")
        for index, step in enumerate(require_list(runs.get("steps"), "the steps"))
    ]
    return {name: named_step(steps, name) for name in MSI_FIXTURE_STEPS}


def test_msi_gate_exercises_the_release_authoring_and_upgrade_path() -> None:
    """Build and install custom authoring through the dedicated MSI merge gate.

    XML parsing cannot reject authoring WiX no longer supports, and compilation
    cannot prove Windows Installer's replacement behaviour. The local action
    therefore builds the same custom WXS with the release packaging action and
    executes the beta-to-beta, beta-to-final, and downgrade transition suite.

    The release lane's pin is read, not spelled out, so Dependabot can bump it;
    what must hold is that the fixtures build with whatever revision the release
    packages with, which nothing else checks.
    """
    msi_steps = job_steps(load_workflow(CI_WINDOWS_WORKFLOW_PATH), MSI_JOB)
    validation_step = named_step(msi_steps, "Validate Windows MSI upgrade paths")
    release_step = named_step(
        job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build"),
        "Build Windows installer package",
    )
    assert validation_step.get("uses") == MSI_VALIDATION_ACTION, (
        "the MSI merge gate must invoke the repository-owned integration action, "
        f"got {validation_step.get('uses')!r}"
    )
    release_sha = _windows_package_sha(
        release_step, "the release lane's Windows installer packaging"
    )
    fixture_steps = _msi_fixture_steps()
    fixture_shas = {
        name: _windows_package_sha(step, f"the {name} step")
        for name, step in fixture_steps.items()
    }
    assert fixture_shas == dict.fromkeys(MSI_FIXTURE_STEPS, release_sha), (
        "every MSI fixture step must build with the same "
        f"{WINDOWS_PACKAGE_ACTION} revision the release lane packages with, "
        f"got fixtures {fixture_shas!r} against release {release_sha!r}"
    )


def test_msi_fixtures_preserve_the_release_authoring_contract() -> None:
    """Each MSI fixture step must keep its version and WiX inputs.

    The versions are what make the sequence ordered — beta1 before beta2
    before final — so a fixture that silently reused another's version would
    leave an upgrade path untested while the suite still passed. Reading them
    from the parsed ``with`` block ties each version to its own step, which a
    text scan of the file cannot do.
    """
    fixtures = _msi_fixture_steps()
    for name, version in MSI_FIXTURE_VERSIONS.items():
        inputs = require_mapping(fixtures[name].get("with"), f"{name} inputs")
        assert inputs.get("wxs-path") == "installer/Package.wxs", (
            f"{name} must build the release custom authoring, "
            f"got {inputs.get('wxs-path')!r}"
        )
        assert inputs.get("wix-extension-version") == "7", (
            f"{name} must use the WiX extension version the release uses, "
            f"got {inputs.get('wix-extension-version')!r}"
        )
        assert inputs.get("version") == version, (
            f"{name} must build version {version!r} so the upgrade sequence "
            f"stays ordered, got {inputs.get('version')!r}"
        )


@pytest.mark.parametrize("script", MSI_VALIDATION_SCRIPTS)
def test_msi_gate_invokes_its_repository_owned_scripts(script: str) -> None:
    """The gate must install, upgrade, and clean up through its own scripts.

    Both scripts are repository-owned, and each covers a direction the other
    does not: the validation script drives the transitions, the cleanup script
    removes whatever the run installed. Dropping either leaves the gate unable
    to report, or unable to leave the runner clean for the next job.
    """
    assert script in MSI_VALIDATION_ACTION_PATH.read_text(encoding="utf-8"), (
        f"the MSI integration action must invoke {script!r}; without it the "
        "gate cannot exercise the transitions or leave a clean runner"
    )
