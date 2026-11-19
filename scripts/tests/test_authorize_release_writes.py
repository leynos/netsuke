"""Specify the release-authorization adapter and its command-line boundary.

``scripts/authorize_release_writes.py`` translates the workflow step's raw
environment into the typed context ``release_write_policy`` decides over, then
renders the decision for ``GITHUB_OUTPUT``. Two contracts meet here: a raw
spelling GitHub cannot emit is refused at this boundary instead of being
guessed at, and the step's command-line entry point writes the outputs, the
notice, and the exit status the workflow reads.

The command-line tests spawn a child process with a complete environment.
That keeps the parent test process's environment untouched, which the
repository's testing standard requires, and exercises the same entry point a
live run uses.
"""

import dataclasses
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the CLI boundary.
import sys
from pathlib import Path

import authorize_release_writes as adapter
import pytest
import release_write_policy as policy

REPOSITORY = "leynos/netsuke"
TAG_REF = "refs/tags/v1.2.3"
BRANCH_REF = "refs/heads/main"
DIRECT_WORKFLOW = f"{REPOSITORY}/.github/workflows/release.yml@{TAG_REF}"
OTHER_WORKFLOW = f"{REPOSITORY}/.github/workflows/nightly.yml@{TAG_REF}"
SCRIPT = Path(__file__).resolve().parents[1] / "authorize_release_writes.py"


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        pytest.param("", False, id="absent"),
        pytest.param("true", True, id="true"),
        pytest.param("false", False, id="false"),
    ],
)
def test_supported_spellings_parse_to_flags(value: str, *, expected: bool) -> None:
    """Parse the only values GitHub materializes for a boolean input."""
    assert adapter.parse_boolean("publish", value) is expected, (
        f"{value!r} should parse to {expected!r}"
    )


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("yes", id="yes"),
        pytest.param("1", id="one"),
        pytest.param("TRUE", id="uppercase"),
        pytest.param("on", id="on"),
    ],
)
def test_malformed_spellings_are_refused(value: str) -> None:
    """Refuse a spelling GitHub cannot emit instead of guessing its meaning."""
    with pytest.raises(adapter.ReleaseInputError, match="publish"):
        adapter.parse_boolean("publish", value)


_DIRECT_FACTS = adapter.InvocationFacts(
    has_caller_inputs=False,
    event_name="push",
    ref=TAG_REF,
    workflow_ref=DIRECT_WORKFLOW,
    repository=REPOSITORY,
)


@pytest.mark.parametrize(
    ("facts", "expected"),
    [
        pytest.param(_DIRECT_FACTS, policy.Origin.DIRECT, id="direct-tag-push"),
        pytest.param(
            dataclasses.replace(
                _DIRECT_FACTS, has_caller_inputs=True, workflow_ref=OTHER_WORKFLOW
            ),
            policy.Origin.REUSABLE,
            id="materialized-caller-inputs",
        ),
        pytest.param(
            dataclasses.replace(
                _DIRECT_FACTS, event_name="pull_request", ref="refs/pull/1/merge"
            ),
            policy.Origin.REUSABLE,
            id="non-push-event",
        ),
        pytest.param(
            dataclasses.replace(_DIRECT_FACTS, workflow_ref=OTHER_WORKFLOW),
            policy.Origin.REUSABLE,
            id="other-workflow",
        ),
        pytest.param(
            dataclasses.replace(_DIRECT_FACTS, workflow_ref="release.yml"),
            policy.Origin.REUSABLE,
            id="unparsable-workflow-ref-fails-closed",
        ),
        pytest.param(
            dataclasses.replace(_DIRECT_FACTS, ref=""),
            policy.Origin.REUSABLE,
            id="missing-ref-fails-closed",
        ),
        pytest.param(
            dataclasses.replace(_DIRECT_FACTS, repository=""),
            policy.Origin.REUSABLE,
            id="missing-repository-fails-closed",
        ),
    ],
)
def test_origin_classification(
    facts: adapter.InvocationFacts, expected: policy.Origin
) -> None:
    """Classify only this workflow's own tag push as direct."""
    assert adapter.classify_origin(facts) is expected, (
        f"{facts!r} should classify as {expected!r}"
    )


def test_outputs_name_the_decision_the_workflow_reads() -> None:
    """Render the workflow's keys from the policy's typed decision."""
    decision = policy.ReleaseAuthorization(
        origin=policy.Origin.REUSABLE,
        should_publish=False,
        dry_run=True,
        should_upload_workflow_artifacts=False,
        reason="publication denied: the caller requested a dry run",
    )

    outputs = adapter.render_outputs(decision)

    assert outputs["should-publish"] == "false", (
        "the rendered decision must deny publication"
    )
    assert outputs["dry-run"] == "true", "the rendered decision must record dry-run"
    assert outputs["origin"] == "reusable", "the rendered decision must name the origin"


def _run_cli(
    tmp_path: Path, **inputs: str
) -> tuple[subprocess.CompletedProcess[str], Path, Path]:
    """Run the step's entry point in a child process with a complete environment.

    Returns
    -------
    tuple[subprocess.CompletedProcess[str], Path, Path]
        The child's completed process, the output file, and the job summary
        file it was pointed at.
    """
    output = tmp_path / "github-output"
    summary = tmp_path / "github-step-summary"
    env = {
        "PATH": os.environ.get("PATH", ""),
        "GITHUB_OUTPUT": str(output),
        "GITHUB_STEP_SUMMARY": str(summary),
    }
    env.update({f"INPUT_{name.upper()}": value for name, value in inputs.items()})
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        [sys.executable, str(SCRIPT)],
        env=env,
        capture_output=True,
        text=True,
        check=False,
        shell=False,
    )
    return completed, output, summary


def _read_outputs(output: Path) -> dict[str, str]:
    """Read the step outputs the child wrote.

    Returns
    -------
    dict[str, str]
        The written outputs, keyed by their declared names.
    """
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


def test_cli_writes_the_effective_outputs(tmp_path: Path) -> None:
    """Publish the decision where the workflow reads it and log the override."""
    completed, output, summary = _run_cli(tmp_path, **_publish_inputs())
    values = _read_outputs(output)

    assert completed.returncode == 0, (
        f"a supported request should succeed; stderr was {completed.stderr!r}"
    )
    assert values["should-publish"] == "false", (
        "the CLI must write the policy's effective decision"
    )
    assert values["dry-run"] == "false", (
        "the CLI must write the effective dry-run decision"
    )
    assert values["origin"] == policy.Origin.REUSABLE, (
        "the CLI must record the invocation origin"
    )
    assert "::notice title=Release authorization::" in completed.stdout, (
        "every decision must be visible in the workflow annotation log"
    )
    assert "reason-category=publish_not_requested" in completed.stdout, (
        "the annotation must name the bounded reason category"
    )
    assert "reason-category" in summary.read_text(encoding="utf-8"), (
        "the job summary must carry the same bounded decision record"
    )


def test_cli_records_an_authorized_decision_in_the_summary(tmp_path: Path) -> None:
    """Record an authorized run too, not only the denials."""
    completed, _, summary = _run_cli(tmp_path, **_publish_inputs(publish="true"))
    recorded = summary.read_text(encoding="utf-8")

    assert completed.returncode == 0, (
        f"an authorized request should succeed; stderr was {completed.stderr!r}"
    )
    assert "- reason-category: `authorized`" in recorded, (
        f"an authorized decision must be recorded; got {recorded!r}"
    )


def test_cli_allows_a_supported_non_publishing_request(tmp_path: Path) -> None:
    """A denied publication is a supported decision, not a step failure."""
    completed, output, _ = _run_cli(tmp_path, **_publish_inputs(ref=BRANCH_REF))
    values = _read_outputs(output)

    assert completed.returncode == 0, (
        f"a non-publishing request is supported; stderr was {completed.stderr!r}"
    )
    assert values["should-publish"] == "false", (
        "a non-publishing reusable request must be recorded, not refused"
    )


@pytest.mark.parametrize(
    ("overrides", "name"),
    [
        pytest.param({"publish": "sometimes"}, "publish", id="publish-spelling"),
        pytest.param(
            {"resolver_should_publish": "yes"},
            "resolver should-publish",
            id="resolver-spelling",
        ),
    ],
)
def test_cli_rejects_a_malformed_input(
    tmp_path: Path, overrides: dict[str, str], name: str
) -> None:
    """A malformed input fails the step with a diagnostic, not a guess."""
    completed, output, summary = _run_cli(tmp_path, **_publish_inputs(**overrides))

    assert completed.returncode == 2, (
        f"a malformed input must fail the step; stderr was {completed.stderr!r}"
    )
    assert "::error" in completed.stderr, (
        "the failure must be visible as a workflow error annotation"
    )
    assert name in completed.stderr, (
        f"the annotation must name the offending input {name!r}"
    )
    assert not output.exists(), (
        "a refused run must not publish step outputs for a decision it did not make"
    )
    assert not summary.exists(), (
        "a refused run must not record a decision it never made"
    )
