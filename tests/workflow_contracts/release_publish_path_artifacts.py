"""Resolve package upload inputs, matrix names, and platform guards."""

import dataclasses
import fnmatch
import re
import typing as typ

from actions_expression_evaluator import ExpressionIssue
from actions_expressions import UnsupportedExpressionError, evaluate_expression
from workflow_loading import (
    job_steps,
    named_step,
    require_list,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

BUILD_JOBS = {
    "build-linux": "linux",
    "build-windows": "windows",
    "build-macos": "macos",
}
UPLOAD_STEPS = {
    "linux": "Upload Linux artefacts",
    "windows": "Upload Windows artefacts",
    "macos": "Upload macOS artefacts",
}
_EXPRESSION = re.compile(r"\$\{\{\s*(.*?)\s*}}", re.DOTALL)
MISSING = object()


@dataclasses.dataclass(frozen=True, slots=True)
class PackageUploadScenario:
    """Hold one run's expected package flag and output context."""

    name: str
    outputs: dict[str, str]
    expected_upload: bool


@dataclasses.dataclass(frozen=True, slots=True)
class FieldEvaluation:
    """Supply the expression context and reporting destination for one field."""

    contexts: cabc.Mapping[str, object]
    violations: list[str]
    label: str
    job_level: bool = False


@dataclasses.dataclass(frozen=True, slots=True)
class BuildUploadCheck:
    """Hold the workflow inputs needed to inspect one build's artifact upload."""

    release: dict[str, object]
    build_steps: list[dict[str, object]]
    job_name: str
    platform: str
    scenario: PackageUploadScenario
    violations: list[str]


@dataclasses.dataclass(frozen=True, slots=True)
class BuildUploadInput:
    """Carry the platform and package-upload input resolved for one build job."""

    caller_platform: object
    is_enabled: bool


def _as_expression(value: object) -> str:
    """Return an expression string or refuse an unsupported value."""
    match value:
        case bool() as boolean:
            return "true" if boolean else "false"
        case str() as expression:
            return expression
        case _:
            raise UnsupportedExpressionError(
                ExpressionIssue.EXPECTED_EXPRESSION_STRING, repr(value)
            )


def evaluate_field(value: object, check: FieldEvaluation) -> object:
    """Evaluate one workflow field and name any unsupported expression."""
    try:
        return evaluate_expression(
            _as_expression(value), check.contexts, job_level=check.job_level
        )
    except UnsupportedExpressionError as error:
        check.violations.append(f"{check.label}: unsupported expression: {error}")
        return MISSING


def _matrix_rows(job: dict[str, object]) -> list[dict[str, object]]:
    """Resolve a job's explicit matrix include rows."""
    strategy = require_mapping(job.get("strategy"), "build job strategy")
    matrix = require_mapping(strategy.get("matrix"), "build job matrix")
    rows = require_list(matrix.get("include"), "build job matrix.include")
    return [require_mapping(row, "build job matrix include row") for row in rows]


def _render_template(template: object, contexts: cabc.Mapping[str, object]) -> str:
    """Interpolate every Actions expression and refuse unresolved references."""
    if not isinstance(template, str):
        raise UnsupportedExpressionError(ExpressionIssue.INVALID_ARTIFACT_NAME)
    rendered: list[str] = []
    cursor = 0
    for match in _EXPRESSION.finditer(template):
        rendered.append(_checked_literal(template[cursor : match.start()]))
        value = evaluate_expression(match.group(), contexts)
        rendered.append(_artifact_scalar(value))
        cursor = match.end()
    rendered.append(_checked_literal(template[cursor:]))
    return "".join(rendered)


def _checked_literal(fragment: str) -> str:
    """Return literal text unless it contains an unmatched expression delimiter."""
    if "${{" in fragment or "}}" in fragment:
        raise UnsupportedExpressionError(ExpressionIssue.INCOMPLETE_ARTIFACT_EXPRESSION)
    return fragment


def _artifact_scalar(value: object) -> str:
    """Format an accepted expression scalar or refuse a structured value."""
    match value:
        case bool() as boolean:
            return "true" if boolean else "false"
        case str() | int() | float():
            return str(value)
        case _:
            raise UnsupportedExpressionError(ExpressionIssue.INVALID_ARTIFACT_VALUE)


def _check_caller_artifact_names(
    release: dict[str, object], repo_name: str, violations: list[str]
) -> None:
    """Validate artifact names after resolving every caller matrix row."""
    for job_name in BUILD_JOBS:
        job = workflow_job(release, job_name)
        with_values = require_mapping(job.get("with"), f"{job_name}.with")
        for row in _matrix_rows(job):
            label = f"{job_name}.artifact-name"
            try:
                name = _render_template(
                    with_values.get("artifact-name", MISSING),
                    {
                        "matrix": row,
                        "needs": {"metadata": {"outputs": {"repo_name": repo_name}}},
                    },
                )
            except UnsupportedExpressionError as error:
                violations.append(f"{label}: unresolved artifact name: {error}")
                continue
            if not fnmatch.fnmatchcase(name, f"{repo_name}-*"):
                violations.append(f"{label}: {name!r} does not match {repo_name}-*")


def _check_upload_name_forwarding(
    build: dict[str, object], violations: list[str]
) -> None:
    """Require each platform uploader to forward its caller's artifact name."""
    steps = job_steps(build, "build")
    for platform, step_name in UPLOAD_STEPS.items():
        upload = named_step(steps, step_name)
        with_values = require_mapping(upload.get("with"), f"{step_name}.with")
        if with_values.get("name") != "${{ inputs['artifact-name'] }}":
            violations.append(
                f"build.{platform}.artifact-name: upload must use its caller name"
            )


def check_artifact_names(
    release: dict[str, object],
    build: dict[str, object],
    repo_name: str,
    violations: list[str],
) -> None:
    """Check matrix-resolved caller names and each platform's name forwarding."""
    _check_caller_artifact_names(release, repo_name, violations)
    _check_upload_name_forwarding(build, violations)


def check_download_pattern(
    release: dict[str, object], repo_name: str, violations: list[str]
) -> None:
    """Require the release download pattern to select the uploaded names."""
    steps = job_steps(release, "release")
    index = step_index_by_key(steps, "uses", "actions/download-artifact@")
    download = require_mapping(steps[index].get("with"), "download-artifact.with")
    pattern = download.get("pattern", MISSING)
    label = "release.download-pattern"
    if pattern is MISSING:
        violations.append(f"{label}: artifact pattern is missing")
        return
    try:
        resolved = _render_template(
            pattern, {"needs": {"metadata": {"outputs": {"repo_name": repo_name}}}}
        )
    except UnsupportedExpressionError as error:
        violations.append(f"{label}: unresolved artifact pattern: {error}")
        return
    expected = f"{repo_name}-*"
    if resolved != expected:
        violations.append(f"{label}: resolved to {resolved!r}, expected {expected!r}")


def _check_build_package_upload_input(check: BuildUploadCheck) -> BuildUploadInput:
    """Evaluate a build job's package-upload input and return its resolved flag."""
    with_values = require_mapping(
        workflow_job(check.release, check.job_name).get("with"),
        f"{check.job_name}.with",
    )
    caller_platform = with_values.get("platform", MISSING)
    if caller_platform is MISSING:
        check.violations.append(
            f"{check.scenario.name}.build-platform.{check.job_name}: input is missing"
        )
        caller_platform = check.platform
    elif caller_platform != check.platform:
        check.violations.append(
            f"{check.scenario.name}.build-platform.{check.job_name}: resolved to "
            f"{caller_platform!r}, expected {check.platform!r}"
        )
    label = f"{check.scenario.name}.package-upload.{check.job_name}"
    expression = with_values.get("should-upload-workflow-artifacts", MISSING)
    if expression is MISSING:
        check.violations.append(f"{label}: input is missing")
        return BuildUploadInput(caller_platform=caller_platform, is_enabled=False)
    value = evaluate_field(
        expression,
        FieldEvaluation(
            contexts={"needs": {"metadata": {"outputs": check.scenario.outputs}}},
            violations=check.violations,
            label=label,
        ),
    )
    uploaded = value is True
    if uploaded != check.scenario.expected_upload:
        check.violations.append(
            f"{label}: resolved to {value!r}, "
            f"expected {check.scenario.expected_upload!r}"
        )
    return BuildUploadInput(caller_platform=caller_platform, is_enabled=uploaded)


def _check_platform_upload_guards(
    check: BuildUploadCheck, package_input: BuildUploadInput
) -> None:
    """Evaluate each platform uploader for one reusable build call."""
    for upload_platform, step_name in UPLOAD_STEPS.items():
        step = named_step(check.build_steps, step_name)
        guard = step.get("if", True)
        label = (
            f"{check.scenario.name}.platform-upload.{check.job_name}.{upload_platform}"
        )
        value = evaluate_field(
            guard,
            FieldEvaluation(
                contexts={
                    "inputs": {
                        "platform": package_input.caller_platform,
                        "should-upload-workflow-artifacts": package_input.is_enabled,
                    }
                },
                violations=check.violations,
                label=label,
            ),
        )
        should_upload = (
            upload_platform == check.platform and check.scenario.expected_upload
        )
        if value is MISSING or bool(value) != should_upload:
            check.violations.append(
                f"{label}: guard did not resolve to {should_upload!r}"
            )


def check_build_uploads(
    release: dict[str, object],
    build: dict[str, object],
    scenario: PackageUploadScenario,
    violations: list[str],
) -> None:
    """Evaluate every build's package-upload input and platform step guard."""
    build_steps = job_steps(build, "build")
    for job_name, platform in BUILD_JOBS.items():
        check = BuildUploadCheck(
            release=release,
            build_steps=build_steps,
            job_name=job_name,
            platform=platform,
            scenario=scenario,
            violations=violations,
        )
        package_input = _check_build_package_upload_input(check)
        _check_platform_upload_guards(check, package_input)
