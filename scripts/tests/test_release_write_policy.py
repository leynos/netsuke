"""Specify the release-write authorization rule over typed values.

The pinned ``determine-release-modes`` action answers ``should-publish: true``
for every ``push`` before it consults the reusable ``publish`` input. A reusable
caller that inherits a push trigger therefore reaches ``publish-release`` unless
the workflow takes its own decision. These rows pin that decision: a requested
dry run is an unconditional veto, a reusable invocation needs ``publish``, and
only a direct tag push of this workflow may publish on the resolver's word
alone.

The two Hypothesis properties are the security claims rather than samples of
them: no combination of context and resolver output lets a dry run publish, and
no reusable invocation without ``publish`` publishes. The named rows keep the
protocol edges readable beside those properties. Parsing raw workflow strings
and rendering ``GITHUB_OUTPUT`` are the adapter's job; those contracts live in
``test_authorize_release_writes.py``.
"""

import dataclasses

import pytest
import release_write_policy as policy
from hypothesis import given
from hypothesis import strategies as st

TAG_REF = "refs/tags/v1.2.3"
BRANCH_REF = "refs/heads/main"
#: A safe direct tag push that the resolver also selected for publication.
_BASE = policy.ReleaseContext(
    publish=False,
    dry_run=False,
    origin=policy.Origin.DIRECT,
    ref=TAG_REF,
    resolver_should_publish=True,
    resolver_dry_run=False,
    resolver_should_upload_workflow_artifacts=True,
)

_FLAGS = st.booleans()
_CONTEXTS = st.builds(
    policy.ReleaseContext,
    publish=_FLAGS,
    dry_run=_FLAGS,
    origin=st.sampled_from(list(policy.Origin)),
    ref=st.sampled_from([TAG_REF, BRANCH_REF, "", "refs/tags/v1.2"]),
    resolver_should_publish=_FLAGS,
    resolver_dry_run=_FLAGS,
    resolver_should_upload_workflow_artifacts=_FLAGS,
)


def _expected(
    *, should_publish: bool, dry_run: bool, origin: policy.Origin, upload: bool
) -> tuple[bool, bool, policy.Origin, bool]:
    """Name the effective flags a matrix row expects."""
    return (should_publish, dry_run, origin, upload)


_DIRECT_WRITE = _expected(
    should_publish=True, dry_run=False, origin=policy.Origin.DIRECT, upload=True
)
_REUSABLE_DENIED = _expected(
    should_publish=False, dry_run=False, origin=policy.Origin.REUSABLE, upload=True
)
_DENIED_WITH_ARTEFACTS = _expected(
    should_publish=False, dry_run=True, origin=policy.Origin.REUSABLE, upload=False
)


@pytest.mark.parametrize(
    ("context", "expected"),
    [
        pytest.param(_BASE, _DIRECT_WRITE, id="direct-tag-push"),
        pytest.param(
            dataclasses.replace(_BASE, origin=policy.Origin.REUSABLE),
            _REUSABLE_DENIED,
            id="reusable-publish-absent",
        ),
        pytest.param(
            dataclasses.replace(_BASE, origin=policy.Origin.REUSABLE, publish=True),
            _expected(
                should_publish=True,
                dry_run=False,
                origin=policy.Origin.REUSABLE,
                upload=True,
            ),
            id="reusable-tag-push-publish-true",
        ),
        pytest.param(
            dataclasses.replace(
                _BASE, origin=policy.Origin.REUSABLE, publish=True, dry_run=True
            ),
            _DENIED_WITH_ARTEFACTS,
            id="dry-run-overrides-publish-true",
        ),
        pytest.param(
            dataclasses.replace(
                _BASE,
                origin=policy.Origin.REUSABLE,
                publish=True,
                ref=BRANCH_REF,
            ),
            _expected(
                should_publish=False,
                dry_run=False,
                origin=policy.Origin.REUSABLE,
                upload=True,
            ),
            id="reusable-branch-push-publish-true",
        ),
        pytest.param(
            dataclasses.replace(_BASE, resolver_should_publish=False),
            _expected(
                should_publish=False,
                dry_run=False,
                origin=policy.Origin.DIRECT,
                upload=True,
            ),
            id="resolver-does-not-select-publication",
        ),
        pytest.param(
            dataclasses.replace(_BASE, resolver_dry_run=True),
            _expected(
                should_publish=False,
                dry_run=True,
                origin=policy.Origin.DIRECT,
                upload=False,
            ),
            id="resolver-dry-run-vetoes-publication",
        ),
        pytest.param(
            dataclasses.replace(_BASE, resolver_should_upload_workflow_artifacts=False),
            _expected(
                should_publish=True,
                dry_run=False,
                origin=policy.Origin.DIRECT,
                upload=False,
            ),
            id="resolver-upload-flag-is-forwarded",
        ),
    ],
)
def test_authorization_matrix(
    context: policy.ReleaseContext,
    expected: tuple[bool, bool, policy.Origin, bool],
) -> None:
    """Resolve each protocol edge to its effective decision."""
    decision = policy.authorize_release_writes(context)
    actual = (
        decision.should_publish,
        decision.dry_run,
        decision.origin,
        decision.should_upload_workflow_artifacts,
    )

    assert actual == expected, f"{context!r} should decide {expected!r}; got {actual!r}"


def test_denied_reason_names_the_reusable_publish_requirement() -> None:
    """Report why an inherited push did not authorize a release write."""
    decision = policy.authorize_release_writes(
        dataclasses.replace(_BASE, origin=policy.Origin.REUSABLE)
    )

    assert "reusable" in decision.reason, (
        f"the denial must name the reusable rule; got {decision.reason!r}"
    )
    assert "publish=true" in decision.reason, (
        f"the denial must name the publish requirement; got {decision.reason!r}"
    )


def test_allowed_reason_names_the_invocation_origin() -> None:
    """Report which origin authorized the write, so the log is auditable."""
    decision = policy.authorize_release_writes(_BASE)

    assert decision.reason == "publication authorized for a direct release", (
        f"the reason should name the origin; got {decision.reason!r}"
    )
    assert decision.reason_category == "authorized", (
        "an authorized decision has no denial category; got "
        f"{decision.reason_category!r}"
    )


def test_reason_category_names_every_denial_in_policy_order() -> None:
    """Expose a bounded category beside the human-readable reason."""
    decision = policy.authorize_release_writes(
        dataclasses.replace(_BASE, origin=policy.Origin.REUSABLE, dry_run=True)
    )

    assert decision.reason_category == "dry_run,publish_not_requested", (
        f"the category should name both denials; got {decision.reason_category!r}"
    )
    assert decision.denials == (
        policy.Denial.DRY_RUN,
        policy.Denial.PUBLISH_NOT_REQUESTED,
    ), f"the decision should expose its denial codes; got {decision.denials!r}"


@given(context=_CONTEXTS)
def test_dry_run_true_always_denies_writes(
    context: policy.ReleaseContext,
) -> None:
    """No other input or resolver answer can turn a dry run into a write."""
    decision = policy.authorize_release_writes(
        dataclasses.replace(context, dry_run=True)
    )

    assert decision.should_publish is False, (
        "a dry-run request must deny release writes for every context"
    )
    assert decision.dry_run is True, (
        "a dry-run request must keep the run in dry-run mode"
    )


@given(context=_CONTEXTS)
def test_reusable_without_publish_always_denies_writes(
    context: policy.ReleaseContext,
) -> None:
    """A reusable invocation publishes only when it explicitly asks to."""
    decision = policy.authorize_release_writes(
        dataclasses.replace(context, publish=False, origin=policy.Origin.REUSABLE)
    )

    assert decision.should_publish is False, (
        "a reusable invocation without publish must deny release writes"
    )
    assert decision.origin is policy.Origin.REUSABLE, (
        "the decision must report the origin it decided for"
    )
