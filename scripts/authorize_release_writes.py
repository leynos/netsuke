#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5"]
# ///
"""Bind the release step's inputs to the write-authorization policy.

The decision itself lives in ``scripts/release_write_policy.py``, which reads
typed values and touches no environment, file, or workflow key. This module is
the adapter the ``release_authorization`` step runs: it parses the raw reusable
inputs and resolver outputs, classifies the invocation origin from the GitHub
context, renders the decision for ``GITHUB_OUTPUT``, and reports a resolver
override as a workflow annotation.

Malformed inputs are refused here, at the boundary where a spelling can be
checked, rather than being guessed at inside the policy.
"""

import dataclasses
import pathlib
import sys
import typing as typ

import release_write_policy as policy

import cyclopts
from cyclopts import App, Parameter

_TRUE = "true"
_ACCEPTED_BOOLEAN_SPELLINGS = frozenset({"", _TRUE, "false"})
_RELEASE_WORKFLOW_PATH = ".github/workflows/release.yml"

app = App(config=cyclopts.config.Env("INPUT_", command=False))


class ReleaseInputError(ValueError):
    """Report a step input the adapter cannot interpret."""


def parse_boolean(name: str, value: str) -> bool:
    """Parse one workflow value, refusing spellings GitHub cannot emit.

    An empty value means the input was absent, which a direct trigger leaves
    for the reusable inputs; it parses as ``False``.

    Returns
    -------
    bool
        Whether the value asked for the option.

    Raises
    ------
    ReleaseInputError
        If the value is not empty, ``true`` or ``false``.

    Examples
    --------
    >>> parse_boolean("publish", "true")
    True
    >>> parse_boolean("dry-run", "")
    False
    """
    normalized = value.strip()
    if normalized not in _ACCEPTED_BOOLEAN_SPELLINGS:
        message = f"{name} must be empty, 'true' or 'false'; got {value!r}"
        raise ReleaseInputError(message)
    return normalized == _TRUE


@dataclasses.dataclass(frozen=True, slots=True)
class InvocationFacts:
    """Carry the run facts that distinguish a direct release push."""

    has_caller_inputs: bool
    event_name: str
    ref: str
    workflow_ref: str
    repository: str


def classify_origin(facts: InvocationFacts) -> policy.Origin:
    """Classify a run as a direct release push or a reusable invocation.

    A direct run carries no callable inputs, is triggered by a push, and names
    this repository's release workflow at the triggering ref. Every other
    shape, including a missing or unparsable workflow ref, is reusable so the
    stricter policy applies: a reusable caller can inherit a push trigger, so
    the event alone proves nothing, and only ``github.workflow_ref``
    distinguishes the two.

    Returns
    -------
    policy.Origin
        Whether the run was triggered directly or invoked as reusable.

    Examples
    --------
    >>> classify_origin(
    ...     InvocationFacts(
    ...         has_caller_inputs=False,
    ...         event_name="push",
    ...         ref="refs/tags/v1.2.3",
    ...         workflow_ref=(
    ...             "leynos/netsuke/.github/workflows/release.yml@refs/tags/v1.2.3"
    ...         ),
    ...         repository="leynos/netsuke",
    ...     )
    ... )
    <Origin.DIRECT: 'direct'>
    """
    if facts.has_caller_inputs:
        return policy.Origin.REUSABLE
    if facts.event_name != "push":
        return policy.Origin.REUSABLE
    if not facts.ref or not facts.repository:
        return policy.Origin.REUSABLE
    expected_workflow_ref = f"{facts.repository}/{_RELEASE_WORKFLOW_PATH}@{facts.ref}"
    if facts.workflow_ref != expected_workflow_ref:
        return policy.Origin.REUSABLE
    return policy.Origin.DIRECT


def render_outputs(decision: policy.ReleaseAuthorization) -> dict[str, str]:
    """Render the decision as the keys the workflow reads.

    Returns
    -------
    dict[str, str]
        One step-output value per key the workflow consumes.

    Examples
    --------
    >>> render_outputs(
    ...     policy.ReleaseAuthorization(
    ...         policy.Origin.DIRECT, True, False, True, "authorized"
    ...     )
    ... )["should-publish"]
    'true'
    """
    return {
        "should-publish": "true" if decision.should_publish else "false",
        "dry-run": "true" if decision.dry_run else "false",
        "should-upload-workflow-artifacts": (
            "true" if decision.should_upload_workflow_artifacts else "false"
        ),
        "origin": str(decision.origin),
        "reason": decision.reason,
    }


def write_outputs(path: pathlib.Path, decision: policy.ReleaseAuthorization) -> None:
    """Append the decision to the workflow output file."""
    with path.open("a", encoding="utf-8") as output:
        for key, value in render_outputs(decision).items():
            output.write(f"{key}={value}\n")


def resolver_override_notice(
    decision: policy.ReleaseAuthorization,
    context: policy.ReleaseContext,
) -> str | None:
    """Describe a decision that overrode the resolver, or return ``None``.

    Only a disagreement is worth an annotation. A resolver that already denied
    publication needs no explanation in the log, and a run the policy and the
    resolver both deny is ordinary.

    Returns
    -------
    str | None
        A workflow ``::notice`` line, or ``None`` when nothing was overridden.
    """
    overrides: list[str] = []
    if context.resolver_should_publish and not decision.should_publish:
        overrides.append("the resolver selected publication but the policy denied it")
    if not context.resolver_dry_run and decision.dry_run:
        overrides.append("the policy selected dry-run although the resolver did not")
    if not overrides:
        return None
    return (
        "::notice title=Release authorization::"
        + "; ".join(overrides)
        + f". {decision.reason}"
    )


@Parameter(name="*")
@dataclasses.dataclass(frozen=True, slots=True)
class WorkflowInputs:
    """Carry the raw inputs and GitHub context one run supplies."""

    publish: typ.Annotated[str, Parameter(env_var="INPUT_PUBLISH")] = ""
    dry_run: typ.Annotated[str, Parameter(env_var="INPUT_DRY_RUN")] = ""
    event_name: typ.Annotated[str, Parameter(env_var="INPUT_EVENT_NAME")] = ""
    ref: typ.Annotated[str, Parameter(env_var="INPUT_REF")] = ""
    workflow_ref: typ.Annotated[str, Parameter(env_var="INPUT_WORKFLOW_REF")] = ""
    repository: typ.Annotated[str, Parameter(env_var="INPUT_REPOSITORY")] = ""
    resolver_should_publish: typ.Annotated[
        str, Parameter(env_var="INPUT_RESOLVER_SHOULD_PUBLISH")
    ] = ""
    resolver_dry_run: typ.Annotated[
        str, Parameter(env_var="INPUT_RESOLVER_DRY_RUN")
    ] = ""
    resolver_should_upload_workflow_artifacts: typ.Annotated[
        str, Parameter(env_var="INPUT_RESOLVER_SHOULD_UPLOAD_WORKFLOW_ARTIFACTS")
    ] = ""

    def as_context(self) -> policy.ReleaseContext:
        """Convert the adapter's raw inputs to the policy's typed value object.

        A malformed spelling propagates from :func:`parse_boolean` as a
        :class:`ReleaseInputError`, which the command line turns into a failed
        step rather than a guessed decision.

        Returns
        -------
        policy.ReleaseContext
            The parsed inputs and the classified invocation origin.
        """
        return policy.ReleaseContext(
            publish=parse_boolean("publish", self.publish),
            dry_run=parse_boolean("dry-run", self.dry_run),
            origin=classify_origin(
                InvocationFacts(
                    has_caller_inputs=bool(self.publish or self.dry_run),
                    event_name=self.event_name,
                    ref=self.ref,
                    workflow_ref=self.workflow_ref,
                    repository=self.repository,
                )
            ),
            ref=self.ref,
            resolver_should_publish=parse_boolean(
                "resolver should-publish", self.resolver_should_publish
            ),
            resolver_dry_run=parse_boolean("resolver dry-run", self.resolver_dry_run),
            resolver_should_upload_workflow_artifacts=parse_boolean(
                "resolver should-upload-workflow-artifacts",
                self.resolver_should_upload_workflow_artifacts,
            ),
        )


@Parameter(name="*")
@dataclasses.dataclass(frozen=True, slots=True)
class OutputTarget:
    """Name the workflow output file the decision is appended to."""

    output_path: typ.Annotated[str, Parameter(env_var="GITHUB_OUTPUT", required=True)]


@app.default
def export_authorization(
    inputs: WorkflowInputs | None = None,
    *,
    target: OutputTarget,
) -> None:
    """Authorize the run, append the effective outputs, and report overrides.

    A supported non-publishing request is a successful decision, not a failure,
    so only a malformed input exits non-zero.

    Raises
    ------
    SystemExit
        With status 2 when a raw input is malformed.
    """
    inputs = WorkflowInputs() if inputs is None else inputs
    try:
        context = inputs.as_context()
    except ReleaseInputError as error:
        print(f"::error title=Release authorization::{error}", file=sys.stderr)
        raise SystemExit(2) from error

    decision = policy.authorize_release_writes(context)
    write_outputs(pathlib.Path(target.output_path), decision)
    notice = resolver_override_notice(decision, context)
    if notice is not None:
        print(notice)


if __name__ == "__main__":
    app()
