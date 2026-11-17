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
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    read_workflow_document,
    require_list,
    require_mapping,
    workflow_job,
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
#: The frozen beta4 authoring beta1 must be built from.
MSI_LEGACY_FIXTURE = (
    ".github/actions/windows-msi-upgrade-validation/fixtures/"
    "legacy-program-files-x86.wxs"
)
#: Each fixture's WXS. beta1 reproduces the shipped beta4 layout, so the gate
#: proves a real installed payload moves; beta2 and final use the release
#: authoring under test. A beta1 switched to the release authoring would make
#: the sequence start where it is supposed to end, and nothing would move.
MSI_FIXTURE_WXS_PATHS = {
    "Build beta1 MSI fixture": MSI_LEGACY_FIXTURE,
    "Build beta2 MSI fixture": "installer/Package.wxs",
    "Build final MSI fixture": "installer/Package.wxs",
}
#: Both repository-owned scripts the gate drives: the first runs the
#: transitions, the second removes whatever the run installed.
MSI_VALIDATION_SCRIPTS = (
    "windows-msi-upgrade-validation.ps1",
    "windows-msi-upgrade-cleanup.ps1",
)
#: The Windows release targets and the WiX architecture each MSI is built for.
#: Both targets carry a 64-bit payload, so both must resolve the native
#: program files directory rather than the 32-bit one.
EXPECTED_MSI_ARCHITECTURES = {
    "x86_64-pc-windows-msvc": "x64",
    "aarch64-pc-windows-msvc": "arm64",
}


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


def _windows_msi_rows(includes: list[object]) -> dict[object, object]:
    """Return each Windows release target's MSI architecture.

    A duplicate target is refused rather than merged, because building the
    mapping directly lets a later row overwrite an earlier one: a target
    wrongly mapped to ``arm64`` followed by a correct ``x64`` row would read
    back as exactly the expected mapping.

    Returns
    -------
    dict[object, object]
        Each matrix entry's target mapped to its MSI architecture.

    Raises
    ------
    AssertionError
        When two entries declare the same target.
    """
    pairs = [
        (entry.get("target"), entry.get("msi_arch"))
        for entry in includes
        if isinstance(entry, dict)
    ]
    targets = [target for target, _ in pairs]
    if len(targets) != len(set(targets)):
        message = (
            f"the release must declare one row per Windows target, got {targets!r}"
        )
        raise AssertionError(message)
    return dict(pairs)


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
    text scan of the file cannot do. beta1's frozen legacy authoring is part
    of the same contract: it is what gives the sequence a real starting layout
    to migrate from.
    """
    fixtures = _msi_fixture_steps()
    for name, version in MSI_FIXTURE_VERSIONS.items():
        inputs = require_mapping(fixtures[name].get("with"), f"{name} inputs")
        assert inputs.get("wxs-path") == MSI_FIXTURE_WXS_PATHS[name], (
            f"{name} must build {MSI_FIXTURE_WXS_PATHS[name]!r}, "
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


def test_msi_action_builds_every_fixture_for_its_requested_architecture() -> None:
    """Build all three fixtures for the architecture the gate leg requests.

    An MSI built for the wrong architecture still installs and still passes
    payload checks, so a hard-coded ``x64`` in one fixture step would leave the
    Arm64 leg silently testing an x64 package. The default stays in the action
    for direct callers, but each step must forward the input.
    """
    document = require_mapping(
        read_workflow_document(MSI_VALIDATION_ACTION_PATH),
        "the MSI upgrade validation action",
    )
    inputs = require_mapping(document.get("inputs"), "the action's inputs")
    architecture_input = require_mapping(
        inputs.get("architecture"), "the action's architecture input"
    )
    assert architecture_input.get("default") == "x64", (
        "direct callers must keep an x64 default, "
        f"got {architecture_input.get('default')!r}"
    )
    fixtures = _msi_fixture_steps()
    for name in MSI_FIXTURE_STEPS:
        with_ = require_mapping(fixtures[name].get("with"), f"{name} inputs")
        assert with_.get("architecture") == "${{ inputs.architecture }}", (
            f"{name} must build for the requested architecture, "
            f"got {with_.get('architecture')!r}"
        )


def test_msi_gate_forwards_each_matrix_architecture_to_the_action() -> None:
    """Pass the gate leg's architecture into the repository-owned action."""
    msi_steps = job_steps(load_workflow(CI_WINDOWS_WORKFLOW_PATH), MSI_JOB)
    validation_step = named_step(msi_steps, "Validate Windows MSI upgrade paths")
    with_ = require_mapping(validation_step.get("with"), "the validation step inputs")
    assert with_.get("architecture") == "${{ matrix.architecture }}", (
        "the MSI gate must forward its matrix architecture, "
        f"got {with_.get('architecture')!r}"
    )


def test_msi_scripts_receive_paths_through_quoted_environment_values() -> None:
    """Keep generated MSI paths out of PowerShell source text."""
    document = require_mapping(
        read_workflow_document(MSI_VALIDATION_ACTION_PATH),
        "the MSI upgrade validation action",
    )
    runs = require_mapping(document.get("runs"), "the action's runs block")
    steps = [
        require_mapping(step, f"action step {index}")
        for index, step in enumerate(require_list(runs.get("steps"), "the steps"))
    ]
    expected_env = {
        "BETA1_MSI": "${{ steps.package_beta1.outputs.msi-path }}",
        "BETA2_MSI": "${{ steps.package_beta2.outputs.msi-path }}",
        "FINAL_MSI": "${{ steps.package_final.outputs.msi-path }}",
    }
    for step_name in [
        "Exercise MSI upgrade and downgrade paths",
        "Remove MSI upgrade validation installations",
    ]:
        step = named_step(steps, step_name)
        assert step.get("env") == expected_env, (
            f"{step_name} must pass MSI paths through its environment"
        )
        run = str(step.get("run", ""))
        for variable in expected_env:
            assert f'"$env:{variable}"' in run, (
                f"{step_name} must quote the {variable} environment value"
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


def test_release_maps_windows_targets_to_their_msi_architectures() -> None:
    """The release must build each Windows target's MSI for that architecture.

    An MSI built for the wrong architecture would still install, so the
    program-files location could look correct while the payload and package
    targeted the other platform. The mapping is read from the release matrix
    and the forwarding expression from its caller inputs, so a rename or a
    literal `x64` in the package step fails here.
    """
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    build_windows = workflow_job(workflow, "build-windows")
    strategy = require_mapping(
        build_windows.get("strategy"), "jobs.build-windows.strategy"
    )
    matrix = require_mapping(strategy.get("matrix"), "jobs.build-windows.matrix")
    includes = require_list(matrix.get("include"), "jobs.build-windows includes")
    actual = _windows_msi_rows(includes)
    assert actual == EXPECTED_MSI_ARCHITECTURES, (
        "the release must map each Windows target to its MSI architecture, "
        f"got {actual!r} against {EXPECTED_MSI_ARCHITECTURES!r}"
    )
    inputs = require_mapping(build_windows.get("with"), "jobs.build-windows.with")
    assert inputs.get("msi-arch") == "${{ matrix.msi_arch }}", (
        "build-windows must pass its matrix entry to build-and-package.yml, "
        f"got {inputs.get('msi-arch')!r}"
    )


def test_the_release_reader_refuses_a_duplicate_windows_target() -> None:
    """A repeated target must fail rather than overwrite an earlier row.

    Building the mapping directly kept only the last row for a target, so a
    wrongly mapped `x86_64-pc-windows-msvc` entry followed by a correct one
    produced exactly the expected mapping and the contract passed.
    """
    with pytest.raises(AssertionError, match="one row per Windows target"):
        _windows_msi_rows([
            {"target": "x86_64-pc-windows-msvc", "msi_arch": "arm64"},
            {"target": "x86_64-pc-windows-msvc", "msi_arch": "x64"},
        ])


def test_package_workflow_forwards_the_msi_architecture_to_packaging() -> None:
    """The packaging action must receive the architecture of the release target."""
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    step = named_step(job_steps(workflow, "build"), "Build Windows installer package")
    inputs = require_mapping(step.get("with"), "the Windows packaging inputs")
    assert inputs.get("architecture") == "${{ inputs['msi-arch'] }}", (
        "windows-package must build for the caller-selected MSI architecture, "
        f"got {inputs.get('architecture')!r}"
    )
