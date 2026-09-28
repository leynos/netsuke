"""Pin the reusable release workflow's WiX extension version contract.

The release workflow can run from a tag push or as a reusable workflow. Both
paths must pass one resolved version to the Windows package job, with the
reusable input default and the tag-push fallback kept in sync.

Run via ``make test-workflow-contracts``.
"""

import os
import pathlib
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - test the workflow's own shell step.

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


def _step_script(step: dict[str, object]) -> str:
    """Return the resolver step's shell script."""
    match step.get("run"):
        case str() as script:
            return script
        case run:
            pytest.fail(f"{STEP_NAME} must have a shell script, got {run!r}")


def _run_step(
    script: str, version: str, output_path: pathlib.Path
) -> subprocess.CompletedProcess[str]:
    """Run the checked-in resolver script with only its shell inputs."""
    environment = {
        "GITHUB_OUTPUT": str(output_path),
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "WIX_EXTENSION_VERSION": version,
    }
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
        ["bash", "-c", script],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        check=False,
        capture_output=True,
        env=environment,
        text=True,
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
    expression = env.get("WIX_EXTENSION_VERSION")

    assert wix_input.get("default") == DEFAULT_VERSION, (
        f"workflow_call must default {INPUT_NAME} to {DEFAULT_VERSION!r}, "
        f"got {wix_input.get('default')!r}"
    )
    assert step.get("id") == STEP_ID, f"{STEP_NAME} must retain id {STEP_ID!r}"
    assert _collapse_whitespace(expression) == EXPECTED_EXPRESSION, (
        "WIX_EXTENSION_VERSION must resolve the reusable input with its "
        f"workflow_call default, got {expression!r}"
    )


def test_the_step_does_not_read_event_payloads(
    release_workflow: dict[str, object],
) -> None:
    """Keep caller event details out of the resolver and its environment."""
    step = _version_step(release_workflow)
    env = require_mapping(step.get("env"), f"{STEP_NAME} environment")
    run = _step_script(step)
    forbidden = ("GITHUB_EVENT_PATH", "github.event_name", "event_name", "jq", "${{")
    for fragment in forbidden:
        assert fragment not in run, (
            f"{STEP_NAME} must not use {fragment!r} in its shell script"
        )

    # The pinned env expression is required there; expression syntax is banned
    # in `run`, where GitHub would interpolate it into shell source.
    env_text = "\n".join(f"{key}: {value}" for key, value in env.items())
    for fragment in forbidden[:-1]:
        assert fragment not in env_text, (
            f"{STEP_NAME} environment must not use {fragment!r}"
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
    ("inputs", "expected_version"),
    [
        pytest.param({INPUT_NAME: "6"}, "6", id="supplied-input"),
        pytest.param({"another-input": "present"}, DEFAULT_VERSION, id="input-absent"),
        pytest.param({INPUT_NAME: ""}, DEFAULT_VERSION, id="input-empty"),
        pytest.param({}, DEFAULT_VERSION, id="tag-push-has-no-inputs"),
    ],
)
def test_resolved_version_reaches_the_step_output(
    tmp_path: pathlib.Path,
    release_workflow: dict[str, object],
    inputs: dict[str, object],
    expected_version: str,
) -> None:
    """Pass supplied and defaulted inputs through the checked-in shell step."""
    step = _version_step(release_workflow)
    env = require_mapping(step.get("env"), f"{STEP_NAME} environment")
    expression = str(env.get("WIX_EXTENSION_VERSION", ""))
    resolved_version = _resolve_input_expression(expression, inputs)
    output_path = pathlib.Path(tmp_path) / "github-output.txt"

    run = _step_script(step)
    result = _run_step(run, resolved_version, output_path)

    assert result.returncode == 0, f"the resolver failed: {result.stderr!r}"
    output = output_path.read_text(encoding="utf-8")
    assert output.splitlines() == [f"value={expected_version}"], (
        f"the resolver must append exactly one expected output line, got {output!r}"
    )
    assert result.stdout == f"Resolved WiX extension version: {expected_version}\n", (
        f"the resolver must echo its value, got {result.stdout!r}"
    )


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


def test_an_empty_resolved_environment_fails_before_writing_output(
    tmp_path: pathlib.Path, release_workflow: dict[str, object]
) -> None:
    """Fail clearly when the step receives an empty version environment."""
    step = _version_step(release_workflow)
    run = _step_script(step)
    output_path = pathlib.Path(tmp_path) / "github-output.txt"

    result = _run_step(run, "", output_path)

    assert result.returncode != 0, "an empty version must fail"
    assert "WIX_EXTENSION_VERSION must be set" in result.stderr, (
        f"the resolver must explain its failure, got {result.stderr!r}"
    )
    assert not output_path.exists(), "the resolver must fail before appending an output"


@pytest.mark.parametrize(
    "version",
    [
        pytest.param("7\nvalue=8", id="newline"),
        pytest.param("7\rvalue=8", id="carriage-return"),
    ],
)
def test_line_breaks_cannot_create_extra_output_records(
    tmp_path: pathlib.Path,
    release_workflow: dict[str, object],
    version: str,
) -> None:
    """Reject caller values that could add records to GITHUB_OUTPUT."""
    step = _version_step(release_workflow)
    env = require_mapping(step.get("env"), f"{STEP_NAME} environment")
    expression = str(env.get("WIX_EXTENSION_VERSION", ""))
    resolved_version = _resolve_input_expression(expression, {INPUT_NAME: version})
    output_path = pathlib.Path(tmp_path) / "github-output.txt"
    run = _step_script(step)

    result = _run_step(run, resolved_version, output_path)

    assert result.returncode != 0, "line breaks must be rejected"
    assert "must not contain line breaks" in result.stderr, (
        f"the resolver must explain the line-break rejection, got {result.stderr!r}"
    )
    assert not output_path.exists(), "rejected input must not create output records"
