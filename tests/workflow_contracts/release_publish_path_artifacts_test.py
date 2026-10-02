"""Specify artifact-template rendering and artifact-name failures."""

import copy

import pytest
from actions_expression_evaluator import ExpressionIssue
from actions_expressions import UnsupportedExpressionError
from release_publish_path_artifacts import _render_template, check_artifact_names
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)


@pytest.fixture
def artifact_workflows() -> tuple[dict[str, object], dict[str, object]]:
    """Return independently mutable release and package workflows."""
    return (
        load_workflow(RELEASE_WORKFLOW_PATH),
        load_workflow(PACKAGE_WORKFLOW_PATH),
    )


def test_render_template_keeps_a_literal_only_name() -> None:
    """Return a template unchanged when it contains no expressions."""
    assert _render_template("netsuke-release", {}) == "netsuke-release", (
        "a literal-only artifact name should remain unchanged"
    )


def test_render_template_resolves_multiple_expressions() -> None:
    """Interpolate each expression in its original position."""
    rendered = _render_template(
        "${{ inputs.prefix }}-${{ matrix.arch }}",
        {"inputs": {"prefix": "netsuke"}, "matrix": {"arch": "arm64"}},
    )

    assert rendered == "netsuke-arm64", "each artifact-name expression should resolve"


@pytest.mark.parametrize(
    ("value", "expected"), [(True, "true"), (False, "false")], ids=["true", "false"]
)
def test_render_template_formats_boolean_values(value: object, expected: str) -> None:
    """Render boolean expression values using lowercase workflow literals."""
    assert (
        _render_template("${{ inputs.enabled }}", {"inputs": {"enabled": value}})
        == expected
    ), "boolean artifact values should use lowercase workflow literals"


def test_render_template_rejects_a_non_string_template() -> None:
    """Classify a non-string template before trying to parse expressions."""
    with pytest.raises(UnsupportedExpressionError) as caught:
        _render_template(None, {})

    assert caught.value.issue is ExpressionIssue.INVALID_ARTIFACT_NAME, (
        "non-string templates should be classified as invalid artifact names"
    )


@pytest.mark.parametrize(
    "template",
    [
        "}} before ${{ inputs.value }}",
        "prefix ${{ inputs.value }} tail ${{",
    ],
    ids=["incomplete-before-match", "incomplete-tail"],
)
def test_render_template_rejects_incomplete_delimiters(
    template: str,
) -> None:
    """Reject stray expression delimiters around otherwise valid expressions."""
    with pytest.raises(UnsupportedExpressionError) as caught:
        _render_template(template, {"inputs": {"value": "resolved"}})

    assert caught.value.issue is ExpressionIssue.INCOMPLETE_ARTIFACT_EXPRESSION, (
        "stray expression delimiters should fail before rendering"
    )


def test_render_template_rejects_an_unresolved_path() -> None:
    """Preserve fail-closed handling for a path absent from the context."""
    with pytest.raises(UnsupportedExpressionError) as caught:
        _render_template("${{ inputs.missing }}", {"inputs": {}})

    assert caught.value.issue is ExpressionIssue.UNKNOWN_PATH, (
        "unresolved template paths should fail closed"
    )


def test_render_template_rejects_a_non_scalar_result() -> None:
    """Refuse structured expression results instead of stringifying them."""
    with pytest.raises(UnsupportedExpressionError) as caught:
        _render_template("${{ inputs.value }}", {"inputs": {"value": ["arm64"]}})

    assert caught.value.issue is ExpressionIssue.INVALID_ARTIFACT_VALUE, (
        "structured expression results should not be stringified"
    )


def test_artifact_name_check_rejects_an_unresolved_matrix_name(
    artifact_workflows: tuple[dict[str, object], dict[str, object]],
) -> None:
    """Report unresolved matrix paths as artifact-name violations."""
    release, build = copy.deepcopy(artifact_workflows)
    with_values = require_mapping(
        workflow_job(release, "build-linux").get("with"), "build-linux.with"
    )
    with_values["artifact-name"] = (
        "${{ needs.metadata.outputs.repo_name }}-${{ matrix.missing }}"
    )
    violations: list[str] = []

    check_artifact_names(release, build, REPO_ROOT.name, violations)

    assert any(
        violation.startswith("build-linux.artifact-name: unresolved artifact name:")
        for violation in violations
    ), "unknown matrix names should produce an artifact-name violation"


def test_artifact_name_check_rejects_a_name_outside_the_repository_pattern(
    artifact_workflows: tuple[dict[str, object], dict[str, object]],
) -> None:
    """Reject resolved matrix names that do not start with the repository name."""
    release, build = copy.deepcopy(artifact_workflows)
    with_values = require_mapping(
        workflow_job(release, "build-linux").get("with"), "build-linux.with"
    )
    with_values["artifact-name"] = "other-${{ matrix.artefact }}"
    violations: list[str] = []

    check_artifact_names(release, build, REPO_ROOT.name, violations)

    assert any(
        violation.startswith("build-linux.artifact-name:")
        and f"does not match {REPO_ROOT.name}-*" in violation
        for violation in violations
    ), "resolved artifact names must match the repository prefix"


def test_artifact_name_check_requires_upload_name_forwarding(
    artifact_workflows: tuple[dict[str, object], dict[str, object]],
) -> None:
    """Reject a platform upload step that substitutes its caller name."""
    release, build = copy.deepcopy(artifact_workflows)
    upload = named_step(job_steps(build, "build"), "Upload Linux artefacts")
    with_values = require_mapping(upload.get("with"), "Upload Linux artefacts.with")
    with_values["name"] = "${{ inputs['other-name'] }}"
    violations: list[str] = []

    check_artifact_names(release, build, REPO_ROOT.name, violations)

    assert "build.linux.artifact-name: upload must use its caller name" in violations, (
        "uploaders must forward the caller artifact-name input"
    )
