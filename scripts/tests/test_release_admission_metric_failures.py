"""Exercise bounded release-admission metrics on controlled failure paths.

Each case drives the real gate through cmd-mox doubles and asserts on what the
production boundary saw: the native argument vectors, the records, the workflow
outputs, and the exit status the workflow step reads.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_metric_failures.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

import pytest
from release_admission_test_doubles import REVISION
from release_admission_test_scenarios import delay_workflow_run_query
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    Boundaries,
    assert_failure_trace_sequence,
    assert_identifiers_excluded_from_records,
    expected_gate_labels,
    expected_operation_labels,
    install_adapted,
    install_boundaries,
    operation_duration,
    operation_records,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

#: The one-second delay the duration test injects, well inside both the
#: default thirty-second operation bound and the shim's own client timeout.
DELAYED_SECONDS = 1.0


def _assert_malformed_revision_is_a_bounded_mismatch(
    metrics: list[dict[str, object]],
    traces: list[dict[str, object]],
    outputs: dict[str, str],
    revision: str,
) -> None:
    """Assert a malformed revision retains only fixed diagnostic values."""
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)

    operation_metric = operation_records(metrics, "resolve_tag_commit")[-1]
    assert operation_metric["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION["resolve_tag_commit"],
        "resolve_tag_commit",
        "failure",
        "mismatch",
    ), "malformed revisions must use the fixed commit-resolution mismatch labels"
    gate_labels = metrics[-1]["labels"]
    assert gate_labels == expected_gate_labels("failure", "mismatch"), (
        "the gate metric must retain the commit-resolution mismatch category"
    )
    assert outputs["gate-outcome"] == "failure", (
        "malformed revisions must publish a failed gate outcome"
    )
    assert not outputs["candidate-commit"], (
        "a revision that is not a full object ID must publish empty, so a value "
        "carrying a newline cannot forge a second workflow-output line"
    )
    assert (
        outputs["gate-error-category"]
        == typ.cast("dict[str, str]", gate_labels)["error_category"]
        == "mismatch"
    ), "workflow outputs must retain the gate metric's mismatch category"
    assert_failure_trace_sequence(traces, "resolve_tag_commit", "mismatch")

    forbidden = {
        candidate for candidate in (revision, revision.rstrip("\n")) if candidate
    }
    assert_identifiers_excluded_from_records(
        metrics,
        traces,
        identifiers=forbidden,
        subject="malformed revisions",
    )


def _assert_malformed_revision_stops_followup_requests(
    boundaries: Boundaries, revision: str
) -> None:
    """Assert a malformed revision stops before fetch and workflow lookup."""
    assert boundaries.gh.calls == [
        ["api", f"repos/{GITHUB_REPOSITORY}/commits/{revision}", "--jq", ".sha"]
    ], "commit resolution must be the only GitHub call, carrying the raw revision"
    assert boundaries.git.calls == [], (
        "a malformed revision must stop before the bounded fetch"
    )


@pytest.mark.parametrize(
    "revision",
    [
        pytest.param("a" * 40 + "\n", id="sha-newline"),
        pytest.param("\n", id="newline"),
    ],
)
def test_malformed_revision_fails_as_mismatch_before_followup_requests(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    revision: str,
) -> None:
    """Reject newline revisions before fetching or looking up workflow runs.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims answer the admission queries.
    tmp_path
        Isolated output directory for the gate.
    revision
        Malformed SHA text whose trailing newline the API answer loses.

    Notes
    -----
    Contract invariants: the raw revision reaches the API unchanged, the
    comparison is against the stripped answer, and the mismatch stops the chain
    before the fetch and the workflow-run lookup. The run observes rather than
    enforces, because an enforcing run names a tag and the resolution then asks
    about the tag instead of the revision under test.

    The published ``candidate-commit`` is asserted empty as well. That is the
    boundary where the malformed value could do damage rather than merely be
    refused: ``GITHUB_OUTPUT`` is read as ``key=value`` lines, so a revision
    ending in a newline would forge a line if it were published verbatim.
    """
    boundaries = install_boundaries(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment={
            "GITHUB_SHA": revision,
            "NETSUKE_RELEASE_ADMISSION_ENFORCE": "false",
        },
    )

    assert run.result.returncode == 0, (
        "observation must retain the malformed revision's diagnostics"
    )
    _assert_malformed_revision_is_a_bounded_mismatch(
        run.metrics, run.traces, run.outputs, revision
    )
    _assert_malformed_revision_stops_followup_requests(boundaries, revision)


def test_default_observation_retains_missing_evidence_metrics(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify missing evidence is observed without failing the workflow.

    Notes
    -----
    The default must mirror the current workflow's lack of an evidence
    producer, which is why the harness defaults to a missing evidence state.
    """
    install_boundaries(cmd_mox)
    run = run_gate(cmd_mox, tmp_path)

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    record = operation_records(run.metrics, "check_scan_freshness")[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION["check_scan_freshness"],
        "check_scan_freshness",
        "failure",
        "missing_evidence",
    ), "observation must retain the missing-evidence operation result"
    assert run.metrics[-1]["labels"] == expected_gate_labels(
        "failure", "missing_evidence"
    ), "observation must retain the missing-evidence gate result"
    assert run.outputs["gate-outcome"] == "failure", (
        "observation must publish the failed gate outcome"
    )
    assert run.outputs["gate-error-category"] == "missing_evidence", (
        "observation must publish the fixed missing-evidence category"
    )


def test_operation_durations_measure_controlled_delay(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify operation durations remain finite and measure command latency.

    Notes
    -----
    A fixed workflow-run delay must lengthen only that operation's measurement.
    The real clock is in use, so the undelayed run's durations are the genuine
    elapsed times rather than a double's constant.
    """
    boundaries = install_boundaries(cmd_mox)
    baseline = run_gate(cmd_mox, tmp_path, evidence_state="fresh")
    assert all(
        operation_duration(baseline.metrics, operation) > 0
        for operation in CANARY_BY_OPERATION
    ), "every executed operation must record a finite positive duration"

    boundaries.gh.wraps(delay_workflow_run_query(DELAYED_SECONDS))
    delayed = run_gate(cmd_mox, tmp_path / "delayed", evidence_state="fresh")
    delayed_duration = operation_duration(delayed.metrics, "fetch_workflow_run")
    baseline_duration = operation_duration(baseline.metrics, "fetch_workflow_run")
    assert delayed_duration > baseline_duration, (
        "operation duration must increase when its bounded command is delayed"
    )


def test_operating_system_refusal_keeps_the_operations_own_category(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a launch refusal is not reclassified as a timeout.

    Notes
    -----
    GNU ``timeout`` reported ``127`` for a program it could not run and ``126``
    for any other refusal, and ``classify_command_result`` maps neither to
    ``timeout``. A ``gh`` whose file cannot be executed must therefore still be
    ``api_error``, which is the classification a missing adapter depends on.
    """
    adapted = install_adapted(cmd_mox)
    unlaunchable = tmp_path / "not-executable"
    unlaunchable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    run = run_gate(
        cmd_mox,
        tmp_path / "run",
        evidence_state="fresh",
        extra_environment={
            **adapted.variables(cmd_mox),
            "NETSUKE_RELEASE_ADMISSION_GH_ADAPTER": str(unlaunchable),
        },
    )

    record = operation_records(run.metrics, "resolve_tag_commit")[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION["resolve_tag_commit"],
        "resolve_tag_commit",
        "failure",
        "api_error",
    ), "an unlaunchable adapter must keep the operation's own error category"
    assert "release-admission adapter could not be run" in run.result.stderr, (
        "the gate must name the program the operating system refused"
    )
    assert REVISION not in run.result.stderr, (
        "the launch diagnostic must not carry the revision"
    )
