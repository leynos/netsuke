"""Decide whether a release run may write to the GitHub release.

``publish-release`` holds ``contents: write``, creates the draft release, and
fills it from this run's staged bytes. The pinned ``determine-release-modes``
action answers ``should-publish: true`` for every ``push`` before it consults
the reusable ``publish`` input, so a reusable caller that inherited a push
trigger reached the write path even when it asked for no publication.

This module is the workflow's own decision. It is a pure function over typed
values, so the workflow contracts exercise the same rule a live run follows.
``dry_run`` is an unconditional veto; a reusable invocation needs an explicit
``publish``; and only a direct tag push of this very workflow may publish on
the resolver's word alone.

The resolver remains necessary but not sufficient. Its ``should_publish`` value
must agree, and its ``dry_run`` value can veto, but neither can authorize a run
the caller's policy refuses.

Translating the workflow's raw strings into these values, classifying the
invocation origin, and rendering the decision for ``GITHUB_OUTPUT`` all belong
to the adapter in ``scripts/authorize_release_writes.py``, so this rule depends
on no environment variable, file, or workflow key.
"""

import dataclasses
import enum
import re

_VERSION_TAG = re.compile(r"refs/tags/v[^/]+\.[^/]+\.[^/]+")


class Origin(enum.StrEnum):
    """Name how a run reached the release workflow."""

    #: A push that triggered this repository's release workflow directly.
    DIRECT = "direct"
    #: A ``workflow_call`` invocation, including the pull-request rehearsal.
    REUSABLE = "reusable"


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseContext:
    """Carry the typed context one run supplies to the authorization policy.

    ``publish`` and ``dry_run`` are the reusable inputs as the adapter parsed
    them; both are ``False`` on a direct trigger, where they are absent. The
    remaining fields name the origin, the ref, and the resolver's answers.
    """

    publish: bool
    dry_run: bool
    origin: Origin
    ref: str
    resolver_should_publish: bool
    resolver_dry_run: bool
    resolver_should_upload_workflow_artifacts: bool


class Denial(enum.StrEnum):
    """Name the closed set of reasons publication may be refused.

    A bounded vocabulary is what lets the adapter record a decision in the run
    log and the job summary without echoing caller-supplied text: each member
    is a fixed token, so no denial can widen into free-form content.
    """

    DRY_RUN = "dry_run"
    PUBLISH_NOT_REQUESTED = "publish_not_requested"
    REF_NOT_A_VERSION_TAG = "ref_not_a_version_tag"
    RESOLVER_DECLINED = "resolver_declined"
    RESOLVER_DRY_RUN = "resolver_dry_run"


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseAuthorization:
    """Record the effective release mode and the reason for it."""

    origin: Origin
    should_publish: bool
    dry_run: bool
    should_upload_workflow_artifacts: bool
    reason: str
    #: The denial codes behind ``reason``, in policy order. A decision that
    #: authorized the write denies nothing, so the tuple is then empty.
    denials: tuple[Denial, ...] = ()

    @property
    def reason_category(self) -> str:
        """Render the bounded category an operator's run log can be read by.

        Returns
        -------
        str
            ``authorized``, or the denial codes in policy order joined by
            commas. The value comes from :class:`Denial` alone, so it is
            bounded by the policy's own vocabulary.
        """
        return ",".join(self.denials) if self.denials else "authorized"


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


def _denials(context: ReleaseContext) -> list[tuple[Denial, str]]:
    """List every reason this run may not write to the release.

    The rules are cumulative so the reported reason explains the whole refusal;
    an empty list means publication is authorized. Each code is paired with the
    sentence that explains it, so the bounded category and the human-readable
    reason are decided together and cannot drift apart.

    Returns
    -------
    list[tuple[Denial, str]]
        Denial codes with their explanations, in policy order.
    """
    denials: list[tuple[Denial, str]] = []
    if context.dry_run:
        denials.append((Denial.DRY_RUN, "the caller requested a dry run"))
    if context.origin is Origin.REUSABLE and not context.publish:
        denials.append((
            Denial.PUBLISH_NOT_REQUESTED,
            "a reusable invocation requires publish=true",
        ))
    if not _is_version_tag(context.ref):
        denials.append((Denial.REF_NOT_A_VERSION_TAG, "the ref is not a v*.*.* tag"))
    if not context.resolver_should_publish:
        denials.append((
            Denial.RESOLVER_DECLINED,
            "the release-mode resolver did not select publication",
        ))
    if context.resolver_dry_run:
        denials.append((
            Denial.RESOLVER_DRY_RUN,
            "the release-mode resolver requested a dry run",
        ))
    return denials


def authorize_release_writes(context: ReleaseContext) -> ReleaseAuthorization:
    """Decide whether this run may write to the GitHub release.

    The policy is applied in order: veto a requested dry run, require
    ``publish`` for a reusable run, require a ``v*.*.*`` tag and a resolver that
    selected publication, then honour a resolver dry run as a final veto.

    Returns
    -------
    ReleaseAuthorization
        The invocation origin, effective publication and dry-run flags, the
        effective workflow-artefact upload flag, a human-readable reason, and
        the denial codes that reason explains.

    Examples
    --------
    >>> context = ReleaseContext(
    ...     publish=False,
    ...     dry_run=False,
    ...     origin=Origin.REUSABLE,
    ...     ref="refs/tags/v1.2.3",
    ...     resolver_should_publish=True,
    ...     resolver_dry_run=False,
    ...     resolver_should_upload_workflow_artifacts=True,
    ... )
    >>> authorize_release_writes(context).should_publish
    False
    >>> direct = dataclasses.replace(context, origin=Origin.DIRECT)
    >>> authorize_release_writes(direct).should_publish
    True
    """
    denials = _denials(context)
    effective_dry_run = context.dry_run or context.resolver_dry_run
    should_publish = not denials
    origin = context.origin
    reason = (
        f"publication authorized for a {origin} release"
        if should_publish
        else "publication denied: " + "; ".join(message for _, message in denials)
    )
    # Diagnostic artefacts are a rehearsal aid, never a release write. A dry
    # run suppresses them, matching the input's documented contract, and
    # otherwise the resolver's own choice is forwarded unchanged.
    should_upload_workflow_artifacts = (
        False
        if effective_dry_run
        else context.resolver_should_upload_workflow_artifacts
    )
    return ReleaseAuthorization(
        origin=origin,
        should_publish=should_publish,
        dry_run=effective_dry_run,
        should_upload_workflow_artifacts=should_upload_workflow_artifacts,
        reason=reason,
        denials=tuple(code for code, _ in denials),
    )
