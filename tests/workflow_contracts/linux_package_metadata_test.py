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
