"""Pin the reusable release workflow's WiX extension version contract.

The release workflow can run from a tag push or as a reusable workflow. Both
paths pass one resolved version to the Windows package job, with a shared
fallback for callers that omit the input.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

STEP_NAME = "Resolve WiX extension version"
STEP_ID = "wix_extension_version"
INPUT_NAME = "wix-extension-version"
DEFAULT_VERSION = "7"
EXPECTED_EXPRESSION = "${{ inputs.wix-extension-version || '7' }}"
EXPECTED_RUN = (
    "uv run --no-project --python 3.14 scripts/resolve_wix_extension_version.py"
)


def _collapse_whitespace(value: object) -> str:
    """Return a value with runs of whitespace reduced to one space."""
    return " ".join(str(value or "").split())


def _resolve_input_expression(expression: str, inputs: dict[str, object]) -> str:
    """Resolve the workflow's one-input, string-default expression shape."""
    match = re.fullmatch(
        r"\$\{\{\s*inputs\.([A-Za-z_][A-Za-z0-9_-]*)\s*\|\|\s*'([^']*)'\s*\}\}",
        _collapse_whitespace(expression),
    )
    if match is None:
        raise ValueError("shape")

    input_name, fallback = match.groups()
    match inputs.get(input_name):
        case None | "":
            return fallback
        case str() as value:
            return value
        case _:
            raise TypeError("type")


@pytest.fixture
def release_workflow() -> dict[str, object]:
    """Load the release workflow for its input and output contracts."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _workflow_triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return release triggers from either PyYAML representation of `on`."""
    triggers = workflow.get("on", workflow.get(True))
    return require_mapping(triggers, "release triggers")


def _workflow_call(workflow: dict[str, object]) -> dict[str, object]:
    """Return the reusable workflow's call trigger configuration."""
    triggers = _workflow_triggers(workflow)
    return require_mapping(triggers.get("workflow_call"), "workflow_call")


def _version_step(workflow: dict[str, object]) -> dict[str, object]:
    """Return the release metadata step that resolves the WiX version."""
    return named_step(job_steps(workflow, "metadata"), STEP_NAME)


@pytest.mark.parametrize(
    ("inputs", "expected_version"),
    [
        pytest.param({INPUT_NAME: "6"}, "6", id="supplied-input"),
        pytest.param({"another-input": "present"}, DEFAULT_VERSION, id="absent"),
        pytest.param({INPUT_NAME: ""}, DEFAULT_VERSION, id="empty"),
        pytest.param({}, DEFAULT_VERSION, id="tag-push-has-no-inputs"),
    ],
)
def test_the_step_expression_resolves_supplied_and_default_versions(
    release_workflow: dict[str, object],
    inputs: dict[str, object],
    expected_version: str,
) -> None:
    """Resolve supplied and missing reusable-workflow inputs consistently."""
    env = require_mapping(
        _version_step(release_workflow).get("env"), f"{STEP_NAME} environment"
    )
    expression = str(env.get("INPUT_WIX_EXTENSION_VERSION", ""))

    assert _resolve_input_expression(expression, inputs) == expected_version, (
        "the step expression must resolve to the supplied or default version"
    )


def test_the_input_default_matches_the_step_expression(
    release_workflow: dict[str, object],
) -> None:
    """Keep the reusable input default and step fallback identical."""
    workflow_call = _workflow_call(release_workflow)
    call_inputs = require_mapping(workflow_call.get("inputs"), "workflow_call inputs")
    wix_input = require_mapping(call_inputs.get(INPUT_NAME), INPUT_NAME)
    step = _version_step(release_workflow)
    env = require_mapping(step.get("env"), f"{STEP_NAME} environment")

    assert wix_input.get("default") == DEFAULT_VERSION, (
        f"workflow_call must default {INPUT_NAME} to {DEFAULT_VERSION!r}, "
        f"got {wix_input.get('default')!r}"
    )
    assert step.get("id") == STEP_ID, f"{STEP_NAME} must retain id {STEP_ID!r}"
    assert _collapse_whitespace(env.get("INPUT_WIX_EXTENSION_VERSION")) == (
        EXPECTED_EXPRESSION
    ), "the resolver must receive the reusable input with its default"


def test_the_step_passes_the_input_to_the_python_resolver(
    release_workflow: dict[str, object],
) -> None:
    """Keep workflow data in the environment and delegate validation to Python."""
    step = _version_step(release_workflow)
    env = require_mapping(step.get("env"), f"{STEP_NAME} environment")

    assert step.get("run") == EXPECTED_RUN, (
        "the workflow must invoke the tested Python resolver"
    )
    assert env == {"INPUT_WIX_EXTENSION_VERSION": EXPECTED_EXPRESSION}, (
        "the resolved version must reach Cyclopts through INPUT_*"
    )


def test_the_metadata_output_exposes_the_resolver_value(
    release_workflow: dict[str, object],
) -> None:
    """Expose the WiX step's value through the metadata job output."""
    metadata = workflow_job(release_workflow, "metadata")
    outputs = require_mapping(metadata.get("outputs"), "metadata outputs")
    expected = "${{ steps." + STEP_ID + ".outputs.value }}"
    assert _collapse_whitespace(outputs.get("wix_extension_version")) == expected, (
        "metadata.wix_extension_version must expose the resolver's value output, "
        f"got {outputs.get('wix_extension_version')!r}"
    )


def test_the_reusable_output_and_windows_call_stay_connected(
    release_workflow: dict[str, object],
) -> None:
    """Pass the metadata version through the workflow output to Windows."""
    workflow_call = _workflow_call(release_workflow)
    call_outputs = require_mapping(
        workflow_call.get("outputs"), "workflow_call outputs"
    )
    wix_output = require_mapping(
        call_outputs.get("wix_extension_version"), "wix output"
    )
    expected = "${{ jobs.metadata.outputs.wix_extension_version }}"
    assert _collapse_whitespace(wix_output.get("value")) == expected, (
        "workflow_call.wix_extension_version must expose the metadata job output, "
        f"got {wix_output.get('value')!r}"
    )

    windows_call = require_mapping(
        workflow_job(release_workflow, "build-windows").get("with"),
        "Windows call inputs",
    )
    assert _collapse_whitespace(windows_call.get(INPUT_NAME)) == (
        "${{ needs.metadata.outputs.wix_extension_version }}"
    ), "the Windows package call must consume the metadata WiX version"


@pytest.mark.parametrize(
    "expression",
    [
        "${{ github.event.inputs.wix-extension-version || '7' }}",
        "${{ inputs.wix-extension-version || inputs.other }}",
        "${{ inputs.wix-extension-version && '7' }}",
    ],
)
def test_the_resolver_rejects_other_expression_shapes(expression: str) -> None:
    """Keep the local evaluator scoped to one input and one string literal."""
    with pytest.raises(ValueError, match="shape"):
        _resolve_input_expression(expression, {INPUT_NAME: "6"})
