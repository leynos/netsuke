"""Specify the release-write authorization boundary.

The pinned ``determine-release-modes`` action answers ``should-publish: true``
for every ``push`` before it consults the reusable ``publish`` input. A reusable
caller that inherits a push trigger therefore reaches ``publish-release`` unless
the workflow takes its own decision. These rows pin that decision: raw
``dry-run: true`` is an unconditional veto, a reusable invocation needs an
explicit ``publish: true``, and only a direct tag push of this workflow may
publish on the resolver's word alone.

The two Hypothesis properties are the security claims rather than samples of
them: no combination of context and resolver output lets a raw dry run publish,
and no reusable invocation without ``publish: true`` publishes. The named rows
keep the protocol edges readable beside those properties.
"""

import dataclasses
import typing as typ

import authorize_release_writes as policy
import pytest
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    from pathlib import Path

REPOSITORY = "leynos/netsuke"
TAG_REF = "refs/tags/v1.2.3"
BRANCH_REF = "refs/heads/main"
DIRECT_WORKFLOW = f"{REPOSITORY}/.github/workflows/release.yml@{TAG_REF}"
OTHER_WORKFLOW = f"{REPOSITORY}/.github/workflows/nightly.yml@{TAG_REF}"
_BASELINE = {
    "publish": "",
    "dry_run": "",
    "event_name": "push",
    "ref": TAG_REF,
    "workflow_ref": DIRECT_WORKFLOW,
    "repository": REPOSITORY,
    "resolver_should_publish": "true",
    "resolver_dry_run": "false",
    "resolver_should_upload_workflow_artifacts": "true",
}

_BOOLEAN = st.sampled_from(["", "true", "false"])
_CONTEXTS = st.builds(
    policy.ReleaseContext,
    publish=_BOOLEAN,
    dry_run=_BOOLEAN,
    event_name=st.sampled_from(["push", "pull_request", "workflow_dispatch", ""]),
    ref=st.sampled_from([TAG_REF, BRANCH_REF, "", "refs/tags/v1.2"]),
    workflow_ref=st.sampled_from([DIRECT_WORKFLOW, OTHER_WORKFLOW, "", "release.yml"]),
    repository=st.sampled_from([REPOSITORY, ""]),
    resolver_should_publish=_BOOLEAN,
    resolver_dry_run=_BOOLEAN,
    resolver_should_upload_workflow_artifacts=_BOOLEAN,
)


def _context(**overrides: str) -> policy.ReleaseContext:
    """Build a policy context from a safe direct-push baseline plus overrides."""
    values: dict[str, str] = dict(_BASELINE)
    values.update(overrides)
    return policy.ReleaseContext(**values)


def _expected(
    *, should_publish: str, dry_run: str, origin: str, upload: str
) -> dict[str, str]:
    """Name the workflow output values a matrix row expects."""
    return {
        "should-publish": should_publish,
        "dry-run": dry_run,
        "origin": origin,
        "should-upload-workflow-artifacts": upload,
    }


_DIRECT_WRITE = _expected(
    should_publish="true", dry_run="false", origin=policy.DIRECT, upload="true"
)
_REUSABLE_DENIED = _expected(
    should_publish="false", dry_run="false", origin=policy.REUSABLE, upload="true"
)
_DENIED_WITH_ARTEFACTS = _expected(
    should_publish="false", dry_run="true", origin=policy.REUSABLE, upload="false"
)


@pytest.mark.parametrize(
    ("overrides", "expected"),
    [
        pytest.param({}, _DIRECT_WRITE, id="direct-tag-push"),
        pytest.param(
            {"publish": "false", "workflow_ref": OTHER_WORKFLOW},
            _REUSABLE_DENIED,
            id="reusable-tag-push-publish-false",
        ),
        pytest.param(
            {"workflow_ref": OTHER_WORKFLOW},
            _REUSABLE_DENIED,
            id="reusable-tag-push-materialized-false",
        ),
        pytest.param(
            {"publish": "true", "workflow_ref": OTHER_WORKFLOW},
            _expected(
                should_publish="true",
                dry_run="false",
                origin=policy.REUSABLE,
                upload="true",
            ),
            id="reusable-tag-push-publish-true",
        ),
        pytest.param(
            {"publish": "true", "dry_run": "true", "workflow_ref": OTHER_WORKFLOW},
            _DENIED_WITH_ARTEFACTS,
            id="reusable-dry-run-overrides-publish-true",
        ),
        pytest.param(
            {"publish": "true", "dry_run": "true"},
            _DENIED_WITH_ARTEFACTS,
            id="raw-dry-run-vetoes-publish",
        ),
        pytest.param(
            {"publish": "true", "ref": BRANCH_REF, "workflow_ref": OTHER_WORKFLOW},
            _REUSABLE_DENIED,
            id="reusable-branch-push-publish-true",
        ),
        pytest.param(
            {"publish": "true", "workflow_ref": "release.yml"},
            _expected(
                should_publish="true",
                dry_run="false",
                origin=policy.REUSABLE,
                upload="true",
            ),
            id="unparsable-workflow-ref-applies-reusable-policy",
        ),
        pytest.param(
            {"publish": "false", "workflow_ref": ""},
            _REUSABLE_DENIED,
            id="missing-workflow-ref-fails-closed",
        ),
        pytest.param(
            {"event_name": "pull_request"},
            _REUSABLE_DENIED,
            id="pull-request-without-dry-run-input-is-reusable",
        ),
        pytest.param(
            {"resolver_should_publish": "false"},
            _expected(
                should_publish="false",
                dry_run="false",
                origin=policy.DIRECT,
                upload="true",
            ),
            id="resolver-does-not-select-publication",
        ),
        pytest.param(
            {"resolver_dry_run": "true"},
            _expected(
                should_publish="false",
                dry_run="true",
                origin=policy.DIRECT,
                upload="false",
            ),
            id="resolver-dry-run-vetoes-publication",
        ),
        pytest.param(
            {"resolver_should_upload_workflow_artifacts": "false"},
            _expected(
                should_publish="true",
                dry_run="false",
                origin=policy.DIRECT,
                upload="false",
            ),
            id="resolver-upload-flag-is-forwarded",
        ),
    ],
)
def test_authorization_matrix(
    overrides: dict[str, str],
    expected: dict[str, str],
) -> None:
    """Resolve each protocol edge to its effective workflow outputs."""
    outputs = policy.render_outputs(
        policy.authorize_release_writes(_context(**overrides))
    )
    actual = {key: outputs[key] for key in expected}

    assert actual == expected, (
        f"{overrides!r} should render {expected!r}; got {actual!r}"
    )


def test_denied_reason_names_the_reusable_publish_requirement() -> None:
    """Report why an inherited push did not authorize a release write."""
    decision = policy.authorize_release_writes(_context(workflow_ref=OTHER_WORKFLOW))

    assert "reusable" in decision.reason, (
        f"the denial must name the reusable rule; got {decision.reason!r}"
    )
    assert "publish=true" in decision.reason, (
        f"the denial must name the publish requirement; got {decision.reason!r}"
    )


def test_allowed_reason_names_the_invocation_origin() -> None:
    """Report which origin authorized the write, so the log is auditable."""
    decision = policy.authorize_release_writes(_context())

    assert decision.reason == "publication authorized for a direct release", (
        f"the reason should name the origin; got {decision.reason!r}"
    )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        pytest.param("publish", "yes", id="publish-yes"),
        pytest.param("publish", "1", id="publish-one"),
        pytest.param("dry_run", "TRUE", id="dry-run-uppercase"),
        pytest.param("dry_run", "on", id="dry-run-on"),
    ],
)
def test_malformed_raw_inputs_are_rejected(field: str, value: str) -> None:
    """Refuse a spelling GitHub cannot emit instead of guessing its meaning."""
    with pytest.raises(policy.ReleasePolicyError, match=field.replace("_", "-")):
        policy.authorize_release_writes(_context(**{field: value}))


@given(context=_CONTEXTS)
def test_raw_dry_run_true_always_denies_writes(
    context: policy.ReleaseContext,
) -> None:
    """No other input or resolver answer can turn a raw dry run into a write."""
    decision = policy.authorize_release_writes(
        dataclasses.replace(context, dry_run="true")
    )

    assert decision.should_publish is False, (
        "a raw dry-run request must deny release writes for every context"
    )
    assert decision.dry_run is True, (
        "a raw dry-run request must keep the run in dry-run mode"
    )


@given(context=_CONTEXTS)
def test_reusable_without_publish_true_always_denies_writes(
    context: policy.ReleaseContext,
) -> None:
    """A reusable invocation publishes only when it explicitly asks to."""
    decision = policy.authorize_release_writes(
        dataclasses.replace(context, publish="false", repository=REPOSITORY)
    )

    assert decision.should_publish is False, (
        "a reusable invocation without publish=true must deny release writes"
    )
    assert decision.origin == policy.REUSABLE, (
        "a materialized publish input proves the run is reusable"
    )


def _run_cli(
    monkeypatch: pytest.MonkeyPatch, output: Path, **inputs: str
) -> dict[str, str]:
    """Run the CLI against the given environment and return its step outputs.

    ``monkeypatch`` is the seam the scripting standards prescribe for a script
    that reads its inputs from the environment, and pytest restores every
    variable when the test ends. The policy itself is tested purely; only this
    adapter test needs the environment of the process it runs in.

    Returns
    -------
    dict[str, str]
        The step outputs the adapter wrote, keyed by their declared names.
    """
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))
    for name, value in inputs.items():
        monkeypatch.setenv(f"INPUT_{name.upper()}", value)
    policy.app([], result_action="return_value")
    return dict(
        line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines()
    )


def _publish_inputs(**overrides: str) -> dict[str, str]:
    """Name the environment inputs of a reusable push-triggered invocation."""
    values = {
        "publish": "false",
        "dry_run": "false",
        "event_name": "push",
        "ref": TAG_REF,
        "workflow_ref": OTHER_WORKFLOW,
        "repository": REPOSITORY,
        "resolver_should_publish": "true",
        "resolver_dry_run": "false",
        "resolver_should_upload_workflow_artifacts": "true",
    }
    values.update(overrides)
    return values


def test_cli_writes_the_effective_outputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """Publish the decision where the workflow reads it and log the override."""
    values = _run_cli(monkeypatch, tmp_path / "github-output", **_publish_inputs())

    assert values["should-publish"] == "false", (
        "the CLI must write the policy's effective decision"
    )
    assert values["dry-run"] == "false", (
        "the CLI must write the effective dry-run decision"
    )
    assert values["origin"] == policy.REUSABLE, (
        "the CLI must record the invocation origin"
    )
    assert "::notice" in capsys.readouterr().out, (
        "a resolver override must be visible in the workflow annotation log"
    )


def test_cli_allows_a_supported_non_publishing_request(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A denied publication is a supported decision, not a step failure."""
    values = _run_cli(
        monkeypatch,
        tmp_path / "github-output",
        **_publish_inputs(ref=BRANCH_REF),
    )

    assert values["should-publish"] == "false", (
        "a non-publishing reusable request must be recorded, not refused"
    )


def test_cli_rejects_a_malformed_input(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """A malformed input fails the step with a diagnostic, not a guess."""
    with pytest.raises(SystemExit) as exit_info:
        _run_cli(
            monkeypatch,
            tmp_path / "github-output",
            **_publish_inputs(publish="sometimes"),
        )

    assert exit_info.value.code == 2, "a malformed input must fail the step"
    assert "::error" in capsys.readouterr().err, (
        "the failure must be visible as a workflow error annotation"
    )
