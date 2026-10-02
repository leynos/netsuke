"""Hold the release workflow's Cargo-sourced Linux package metadata path."""

import pytest
from workflow_loading import (
    CI_WORKFLOW_PATH,
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

PACKAGE_METADATA = {
    "maintainer": ("package_maintainer", "package-maintainer"),
    "homepage": ("package_homepage", "package-homepage"),
    "license": ("package_license", "package-license"),
    "description": ("package_description", "package-description"),
}
READER_STEP = "Read Linux package metadata from Cargo.toml"
PACKAGE_ACTION = "leynos/shared-actions/.github/actions/linux-packages"


def _package_inputs() -> dict[str, object]:
    """Return the reusable workflow inputs declared for Linux metadata."""
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    triggers = require_mapping(workflow.get("on"), "build-and-package triggers")
    workflow_call = require_mapping(
        triggers.get("workflow_call"), "build-and-package workflow_call"
    )
    return require_mapping(workflow_call.get("inputs"), "workflow_call inputs")


def test_release_reader_follows_manifest_resolution_and_exposes_outputs() -> None:
    """Read metadata after manifest selection and expose four job outputs."""
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    metadata_job = workflow_job(release, "metadata")
    steps = job_steps(release, "metadata")
    manifest_index = step_index_by_key(steps, "id", "manifest_path")
    reader_index = step_index_by_key(steps, "id", "package_metadata")
    assert manifest_index < reader_index, (
        "metadata reader must follow manifest resolution"
    )

    reader = named_step(steps, READER_STEP)
    assert reader.get("shell") == "bash", "Cargo metadata reader must run in bash"
    environment = require_mapping(reader.get("env"), "metadata reader environment")
    assert environment.get("CARGO_MANIFEST_PATH") == (
        "${{ steps.manifest_path.outputs.value }}"
    ), "metadata reader must use the resolved manifest path"
    match reader.get("run"):
        case str() as command:
            pass
        case _:
            pytest.fail("reader command must be a string")
    assert all(
        fragment in command
        for fragment in (
            "set -euo pipefail",
            "uv run --no-project --python 3.14",
            "scripts/cargo_package_metadata.py",
            '--manifest "$CARGO_MANIFEST_PATH"',
        )
    ), "metadata reader command must validate and read the resolved manifest"

    outputs = require_mapping(metadata_job.get("outputs"), "metadata job outputs")
    assert outputs.get("package_manifest") == (
        "${{ steps.manifest_path.outputs.value }}"
    ), "the resolved manifest path must be available to build-linux"
    for field, (output_name, _) in PACKAGE_METADATA.items():
        expected = "${{ steps.package_metadata.outputs." + field + " }}"
        assert outputs.get(output_name) == expected, (
            f"metadata job must expose {output_name} from the reader"
        )


def test_release_metadata_reader_uses_the_pinned_python_baseline() -> None:
    """Run the metadata script under the pinned Python 3.14 interpreter."""
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), "metadata")
    setup = named_step(steps, "Install Python for package metadata")
    assert str(setup.get("uses", "")).startswith("astral-sh/setup-uv@"), (
        "release metadata must install uv before running the reader"
    )
    setup_inputs = require_mapping(setup.get("with"), "package metadata uv inputs")
    assert setup_inputs.get("python-version") == "3.14", (
        "release metadata must use the Python 3.14 baseline"
    )
    assert setup_inputs.get("enable-cache") is False, (
        "the stdlib-only metadata reader does not need a uv cache"
    )
    assert step_index_by_key(
        steps, "name", "Install Python for package metadata"
    ) < step_index_by_key(steps, "id", "package_metadata"), (
        "uv must be configured before the metadata reader runs"
    )


def test_linux_release_forwards_metadata_without_workflow_call_outputs() -> None:
    """Forward reader outputs through build-linux without widening callers."""
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    build_linux = workflow_job(release, "build-linux")
    forwarded = require_mapping(build_linux.get("with"), "build-linux inputs")
    for output_name, input_name in PACKAGE_METADATA.values():
        expected = "${{ needs.metadata.outputs." + output_name + " }}"
        assert forwarded.get(input_name) == expected, (
            f"build-linux must forward {output_name} into {input_name}"
        )
    assert forwarded.get("package-manifest") == (
        "${{ needs.metadata.outputs.package_manifest }}"
    ), "build-linux must forward the resolved manifest path"

    package_workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    triggers = require_mapping(package_workflow.get("on"), "package workflow triggers")
    workflow_call = require_mapping(
        triggers.get("workflow_call"), "package workflow_call"
    )
    declared_outputs = require_mapping(
        workflow_call.get("outputs", {}), "workflow_call outputs"
    )
    for output_name, _ in PACKAGE_METADATA.values():
        assert output_name not in declared_outputs, (
            "package metadata must remain an internal job output, "
            f"not workflow_call output {output_name}"
        )


def test_package_workflow_inputs_are_optional_and_map_to_linux_action() -> None:
    """Declare optional string inputs and map them to nFPM's metadata fields."""
    inputs = _package_inputs()
    for _, input_name in PACKAGE_METADATA.values():
        declaration = require_mapping(inputs.get(input_name), f"input {input_name}")
        assert declaration.get("required") is False, f"{input_name} must be optional"
        assert declaration.get("type") == "string", f"{input_name} must be a string"
        match declaration.get("default"):
            case str() as default_value:
                assert not default_value, f"{input_name} must default to empty"
            case _:
                pytest.fail(f"{input_name} default must be a string")
    manifest_input = require_mapping(
        inputs.get("package-manifest"), "input package-manifest"
    )
    assert manifest_input.get("required") is False, (
        "the resolved package manifest must remain optional"
    )
    assert manifest_input.get("type") == "string", (
        "the resolved package manifest must be a string"
    )
    assert manifest_input.get("default") == "Cargo.toml", (
        "direct workflow callers must retain the root-manifest default"
    )

    package_step = named_step(
        job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build"),
        "Package Linux artefacts with dependencies",
    )
    assert str(package_step.get("uses", "")).startswith(PACKAGE_ACTION), (
        "Linux package step must use the shared linux-packages action"
    )
    action_inputs = require_mapping(
        package_step.get("with"), "Linux package action inputs"
    )
    for field, (_, input_name) in PACKAGE_METADATA.items():
        expected = "${{ inputs['" + input_name + "'] }}"
        assert action_inputs.get(field) == expected, (
            f"linux-packages must map {field} from {input_name}"
        )


def test_ci_runs_the_linux_package_metadata_pytest_target() -> None:
    """Run the reader and package validator tests from the Linux CI lane."""
    steps = job_steps(load_workflow(CI_WORKFLOW_PATH), "build-test")
    workflow_contract_index = step_index_by_key(
        steps, "name", "Workflow contract tests"
    )
    package_metadata_index = step_index_by_key(
        steps, "run", "make test-linux-package-metadata"
    )
    assert package_metadata_index > workflow_contract_index, (
        "Linux package metadata tests must run after workflow contracts"
    )


def test_linux_package_metadata_gate_runs_after_packaging_before_upload() -> None:
    """Validate Linux package fields after pruning and before artefact upload."""
    steps = job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build")
    step_indices = [
        step_index_by_key(steps, "name", name)
        for name in (
            "Package Linux artefacts with dependencies",
            "Prune packaging metadata",
            "Install Python 3.14 for Linux package metadata validation",
            "Validate Linux package metadata",
            "Upload Linux artefacts",
        )
    ]
    assert step_indices == sorted(set(step_indices)), (
        "metadata validation must follow package creation and pruning and "
        "install its Python runtime before running, then precede Linux "
        "artefact upload"
    )

    python_setup = named_step(
        steps, "Install Python 3.14 for Linux package metadata validation"
    )
    assert python_setup.get("if") == "inputs.platform == 'linux'", (
        "the Python setup must be limited to Linux package jobs"
    )
    assert str(python_setup.get("uses", "")).startswith("astral-sh/setup-uv@"), (
        "the Python baseline must be provisioned by the pinned uv action"
    )
    python_inputs = require_mapping(python_setup.get("with"), "Python setup inputs")
    assert python_inputs.get("python-version") == "3.14", (
        "the package validator must use the repository Python baseline"
    )
    assert python_inputs.get("enable-cache") is False, (
        "the release validator must not share the setup-uv cache"
    )

    validation = named_step(steps, "Validate Linux package metadata")
    assert validation.get("if") == "inputs.platform == 'linux'", (
        "package metadata validation must be Linux-gated"
    )
    assert "continue-on-error" not in validation, (
        "package metadata validation must fail the build when fields differ"
    )
    environment = require_mapping(validation.get("env"), "metadata validator env")
    expected_environment = {
        "PACKAGE_NAME": "${{ inputs['bin-name'] }}",
        "PACKAGE_MAINTAINER": "${{ inputs['package-maintainer'] }}",
        "PACKAGE_HOMEPAGE": "${{ inputs['package-homepage'] }}",
        "PACKAGE_LICENSE": "${{ inputs['package-license'] }}",
        "PACKAGE_DESCRIPTION": "${{ inputs['package-description'] }}",
        "PACKAGE_MANIFEST": "${{ inputs['package-manifest'] }}",
    }
    for name, expression in expected_environment.items():
        assert environment.get(name) == expression, (
            f"the validator must receive the {name} workflow value"
        )
    match validation.get("run"):
        case str() as command:
            pass
        case _:
            pytest.fail("Linux package metadata gate must define a command")
    for fragment in (
        "set -euo pipefail",
        "for field in PACKAGE_MAINTAINER",
        "PACKAGE_DESCRIPTION; do",
        'if [[ -z "${!field}" ]]; then',
        "Missing Linux package metadata",
        "sudo apt-get install --no-install-recommends --yes rpm",
        "uv run --no-project --python 3.14",
        "scripts/validate_linux_package_metadata.py",
        "--dist dist",
        '--manifest "$PACKAGE_MANIFEST"',
        '--package-name "$PACKAGE_NAME"',
        "--license-file LICENSE",
    ):
        assert fragment in command, (
            f"Linux metadata validation command must include {fragment!r}"
        )
    assert (
        command.index('if [[ -z "${!field}" ]]')
        < command.index("sudo apt-get update")
        < command.index("sudo apt-get install")
        < command.index("uv run --no-project --python 3.14")
        < command.index("scripts/validate_linux_package_metadata.py")
    ), "metadata must be checked before installing tools or validating packages"

    assert "always()" not in str(
        named_step(steps, "Upload Linux artefacts").get("if", "")
    ), "Linux artefact upload must not bypass a failed validation step"
