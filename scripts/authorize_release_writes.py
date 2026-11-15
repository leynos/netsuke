#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5"]
# ///
"""Authorize release writes from the reusable policy and the mode resolver.

``publish-release`` holds ``contents: write``, creates the draft release, and
fills it from this run's staged bytes. The pinned ``determine-release-modes``
action answers ``should-publish: true`` for every ``push`` before it consults
the reusable ``publish`` input, so a reusable caller that inherited a push
trigger reached the write path even when it asked for no publication.

This module is the workflow's own decision. It is a pure function over the raw
reusable inputs and the GitHub context, so the workflow contracts exercise the
same rules a live run follows. ``dry-run: true`` is an unconditional veto; a
reusable invocation needs an explicit ``publish: true``; and only a direct tag
push of this very workflow may publish on the resolver's word alone.

The resolver remains necessary but not sufficient. Its ``should-publish`` value
must agree, and its ``dry-run`` value can veto, but neither can authorize a run
the caller's policy refuses.
"""

import dataclasses
import pathlib
import re
import sys
import typing as typ

import cyclopts
from cyclopts import App, Parameter

#: A run triggered directly by a push to this repository's release workflow.
DIRECT = "direct"
#: A run invoked through ``workflow_call``, including the pull-request rehearsal.
REUSABLE = "reusable"
_TRUE = "true"
_ACCEPTED_BOOLEAN_SPELLINGS = frozenset({"", _TRUE, "false"})
_RELEASE_WORKFLOW_PATH = ".github/workflows/release.yml"
_VERSION_TAG = re.compile(r"refs/tags/v[^/]+\.[^/]+\.[^/]+")

app = App(config=cyclopts.config.Env("INPUT_", command=False))


class ReleasePolicyError(ValueError):
    """Report a raw policy input the authorization rule cannot interpret."""


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseContext:
    """Carry the raw context one run supplies to the authorization policy.

    ``publish`` and ``dry_run`` hold the reusable inputs as GitHub materializes
    them: empty on a direct trigger, otherwise ``"true"`` or ``"false"``. The
    remaining fields name the trigger, the caller, and the resolver's answers.
    """

    publish: str
    dry_run: str
    event_name: str
    ref: str
    workflow_ref: str
    repository: str
    resolver_should_publish: str
    resolver_dry_run: str
    resolver_should_upload_workflow_artifacts: str


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseAuthorization:
    """Record the effective release mode and the reason for it."""

    origin: str
    should_publish: bool
    dry_run: bool
    should_upload_workflow_artifacts: bool
    reason: str


def _parse_requested_boolean(name: str, value: str) -> bool:
    """Parse one reusable input, refusing spellings GitHub cannot emit.

    Returns
    -------
    bool
        Whether the input asked for the option; an empty input does not.

    Raises
    ------
    ReleasePolicyError
        If the value is not empty, ``true`` or ``false``.

    Examples
    --------
    >>> _parse_requested_boolean("publish", "true")
    True
    >>> _parse_requested_boolean("dry-run", "")
    False
    """
    normalized = value.strip()
    if normalized not in _ACCEPTED_BOOLEAN_SPELLINGS:
        message = f"{name} must be empty, 'true' or 'false'; got {value!r}"
        raise ReleasePolicyError(message)
    return normalized == _TRUE


def classify_origin(context: ReleaseContext) -> str:
    """Classify a run as a direct release push or a reusable invocation.

    A direct run has no callable inputs, is triggered by a push, and names this
    repository's release workflow at the triggering ref. Every other shape,
    including a missing or unparsable workflow ref, is reusable so the stricter
    policy applies: a reusable caller can inherit a push trigger, so the event
    alone proves nothing, and only ``github.workflow_ref`` distinguishes the
    two.

    Returns
    -------
    str
        ``DIRECT`` or ``REUSABLE``.

    Examples
    --------
    >>> workflow_ref = "leynos/netsuke/.github/workflows/release.yml@refs/tags/v1.2.3"
    >>> direct = ReleaseContext(
    ...     publish="",
    ...     dry_run="",
    ...     event_name="push",
    ...     ref="refs/tags/v1.2.3",
    ...     workflow_ref=workflow_ref,
    ...     repository="leynos/netsuke",
    ...     resolver_should_publish="true",
    ...     resolver_dry_run="false",
    ...     resolver_should_upload_workflow_artifacts="true",
    ... )
    >>> classify_origin(direct)
    'direct'
    >>> classify_origin(dataclasses.replace(direct, publish="false"))
    'reusable'
    """
    if context.publish or context.dry_run:
        return REUSABLE
    if context.event_name != "push":
        return REUSABLE
    if not context.ref or not context.repository:
        return REUSABLE
    expected_workflow_ref = (
        f"{context.repository}/{_RELEASE_WORKFLOW_PATH}@{context.ref}"
    )
    return DIRECT if context.workflow_ref == expected_workflow_ref else REUSABLE


def _is_version_tag(ref: str) -> bool:
    """Return whether the ref names a ``v*.*.*`` tag.

    Returns
    -------
    bool
        Whether the ref is a full match for a three-part version tag.

    Examples
    --------
    >>> _is_version_tag("refs/tags/v1.2.3")
    True
    >>> _is_version_tag("refs/heads/main")
    False
    """
    return _VERSION_TAG.fullmatch(ref) is not None


def _denials(context: ReleaseContext, origin: str) -> list[str]:
    """List every reason this run may not write to the release.

    The rules are cumulative so the reported reason explains the whole refusal;
    an empty list means publication is authorized.

    Returns
    -------
    list[str]
        Human-readable denial reasons, in policy order.
    """
    denials: list[str] = []
    if _parse_requested_boolean("dry-run", context.dry_run):
        denials.append("the caller requested a dry run")
    if origin == REUSABLE and not _parse_requested_boolean("publish", context.publish):
        denials.append("a reusable invocation requires publish=true")
    if not _is_version_tag(context.ref):
        denials.append("the ref is not a v*.*.* tag")
    if context.resolver_should_publish != _TRUE:
        denials.append("the release-mode resolver did not select publication")
    if context.resolver_dry_run == _TRUE:
        denials.append("the release-mode resolver requested a dry run")
    return denials


def authorize_release_writes(context: ReleaseContext) -> ReleaseAuthorization:
    """Decide whether this run may write to the GitHub release.

    The policy is applied in order: reject malformed raw inputs, veto a raw
    ``dry-run: true``, classify the invocation origin, require ``publish: true``
    for a reusable run, require a ``v*.*.*`` tag and a resolver that selected
    publication, then honour a resolver dry-run as a final veto. A malformed raw
    input propagates ``ReleasePolicyError`` rather than being guessed at.

    Returns
    -------
    ReleaseAuthorization
        The invocation origin, effective publication and dry-run flags, the
        effective workflow-artefact upload flag, and a human-readable reason.

    Examples
    --------
    >>> workflow_ref = "leynos/netsuke/.github/workflows/release.yml@refs/tags/v1.2.3"
    >>> context = ReleaseContext(
    ...     publish="false",
    ...     dry_run="",
    ...     event_name="push",
    ...     ref="refs/tags/v1.2.3",
    ...     workflow_ref=workflow_ref,
    ...     repository="leynos/netsuke",
    ...     resolver_should_publish="true",
    ...     resolver_dry_run="false",
    ...     resolver_should_upload_workflow_artifacts="true",
    ... )
    >>> authorize_release_writes(context).should_publish
    False
    >>> direct = dataclasses.replace(context, publish="")
    >>> authorize_release_writes(direct).should_publish
    True
    """
    origin = classify_origin(context)
    denials = _denials(context, origin)
    effective_dry_run = (
        _parse_requested_boolean("dry-run", context.dry_run)
        or context.resolver_dry_run == _TRUE
    )
    should_publish = not denials
    reason = (
        f"publication authorized for a {origin} release"
        if should_publish
        else "publication denied: " + "; ".join(denials)
    )
    # Diagnostic artefacts are a rehearsal aid, never a release write. A dry
    # run suppresses them, matching the input's documented contract, and
    # otherwise the resolver's own choice is forwarded unchanged.
    should_upload_workflow_artifacts = (
        False
        if effective_dry_run
        else context.resolver_should_upload_workflow_artifacts == _TRUE
    )
    return ReleaseAuthorization(
        origin=origin,
        should_publish=should_publish,
        dry_run=effective_dry_run,
        should_upload_workflow_artifacts=should_upload_workflow_artifacts,
        reason=reason,
    )


def render_outputs(decision: ReleaseAuthorization) -> dict[str, str]:
    """Render the decision as the keys the workflow reads.

    Returns
    -------
    dict[str, str]
        One ``step output`` value per key the workflow consumes.

    Examples
    --------
    >>> render_outputs(
    ...     ReleaseAuthorization("direct", True, False, True, "authorized")
    ... )["should-publish"]
    'true'
    """
    return {
        "should-publish": "true" if decision.should_publish else "false",
        "dry-run": "true" if decision.dry_run else "false",
        "should-upload-workflow-artifacts": (
            "true" if decision.should_upload_workflow_artifacts else "false"
        ),
        "origin": decision.origin,
        "reason": decision.reason,
    }


def write_outputs(path: pathlib.Path, decision: ReleaseAuthorization) -> None:
    """Append the decision to the workflow output file."""
    with path.open("a", encoding="utf-8") as output:
        for key, value in render_outputs(decision).items():
            output.write(f"{key}={value}\n")


def resolver_override_notice(
    decision: ReleaseAuthorization,
    context: ReleaseContext,
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
    if context.resolver_should_publish == _TRUE and not decision.should_publish:
        overrides.append("the resolver selected publication but the policy denied it")
    if context.resolver_dry_run != _TRUE and decision.dry_run:
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

    def as_context(self) -> ReleaseContext:
        """Convert the adapter's inputs to the policy's own value object."""
        return ReleaseContext(
            publish=self.publish,
            dry_run=self.dry_run,
            event_name=self.event_name,
            ref=self.ref,
            workflow_ref=self.workflow_ref,
            repository=self.repository,
            resolver_should_publish=self.resolver_should_publish,
            resolver_dry_run=self.resolver_dry_run,
            resolver_should_upload_workflow_artifacts=(
                self.resolver_should_upload_workflow_artifacts
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
        With status 2 when a raw reusable input is malformed.
    """
    inputs = WorkflowInputs() if inputs is None else inputs
    context = inputs.as_context()
    try:
        decision = authorize_release_writes(context)
    except ReleasePolicyError as error:
        print(f"::error title=Release authorization::{error}", file=sys.stderr)
        raise SystemExit(2) from error

    write_outputs(pathlib.Path(target.output_path), decision)
    notice = resolver_override_notice(decision, context)
    if notice is not None:
        print(notice)


if __name__ == "__main__":
    app()
