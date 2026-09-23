r"""Verify release-admission metrics stay bounded.

The fixed failure-category matrix pins operation and category pairs to bounded
labels. The Hypothesis property uses canonical Git object IDs to reach commit
and workflow-run requests, then keeps generated identifiers out of metric
labels and trace fields.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        python -m pytest \
        scripts/tests/test_release_admission_metric_boundedness.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import tempfile
import typing as typ
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    FailureCase,
    _run_gate,
    assert_failure_trace_sequence,
    expected_gate_labels,
    expected_operation_labels,
    operation_duration,
    operation_records,
)

IDENTIFIER_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=1,
    max_size=32,
)
GIT_OBJECT_ID = st.one_of(
    st.text(alphabet="0123456789abcdef", min_size=40, max_size=40),
    st.text(alphabet="0123456789abcdef", min_size=64, max_size=64),
)


def _assert_diagnostics_cross_boundaries(
    calls: list[dict[str, object]],
    expected_diagnostics: dict[str, str],
) -> None:
    """Verify generated path and URL diagnostics cross every fake boundary."""
    assert calls, "the fake command adapters must be invoked"
    assert all(call["diagnostics"] == expected_diagnostics for call in calls), (
        "generated paths and URLs must cross each fake command boundary"
    )


def _assert_github_requests_cross_boundary(
    calls: list[dict[str, object]],
    repository: str,
    revision: str,
    error_category: str,
) -> None:
    """Verify exact GitHub requests reach the fake GitHub boundary."""
    github_calls = [call for call in calls if call["command"] == "gh"]
    github_arguments = [
        typ.cast("list[str]", call["arguments"]) for call in github_calls
    ]
    diagnostic = (
        f"gh call log: {github_calls!r}; final gate error category: {error_category!r}"
    )
    assert error_category == "missing_evidence", (
        f"the request-success path must reach the expected evidence check; {diagnostic}"
    )
    assert github_arguments[:1] == [
        ["api", f"repos/{repository}/commits/{revision}", "--jq", ".sha"]
    ], f"the exact commit-resolution request must cross the boundary; {diagnostic}"
    assert github_arguments[1:] == [
        [
            "api",
            f"repos/{repository}/actions/runs?head_sha={revision}&per_page=1",
            "--jq",
            ".workflow_runs[0].id // empty",
        ]
    ], f"the exact workflow-run request must cross the boundary; {diagnostic}"


def _assert_identifiers_are_excluded(
    metrics: list[dict[str, object]],
    traces: list[dict[str, object]],
    identifiers: set[str],
) -> None:
    """Verify generated identifiers are absent from metric labels and traces."""
    for record in metrics:
        labels = record["labels"]
        assert isinstance(labels, dict), "every emitted metric must retain labels"
        assert identifiers.isdisjoint(labels.values()), (
            "generated identifiers must never become metric label values"
        )
    for trace in traces:
        assert identifiers.isdisjoint(trace.values()), (
            "generated identifiers must never become trace field values"
        )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_FAILURE": "true"},
                "resolve_tag_commit",
                "api_error",
            ),
            id="api-error",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_RESOLVED_REVISION": "b" * 40},
                "resolve_tag_commit",
                "mismatch",
            ),
            id="candidate-mismatch",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GIT_FAILURE": "true"},
                "fetch_candidate_revision",
                "fetch_error",
            ),
            id="fetch-error",
        ),
        pytest.param(
            FailureCase("stale", {}, "check_scan_freshness", "stale_evidence"),
            id="stale-evidence",
        ),
        pytest.param(
            FailureCase("missing", {}, "check_scan_freshness", "missing_evidence"),
            id="missing-evidence",
        ),
        pytest.param(
            FailureCase("unexpected", {}, "check_scan_freshness", "unknown"),
            id="unknown-evidence",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {},
                "verify_evidence",
                "missing_evidence",
            ),
            id="enforcement-rejects-environment-freshness",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "resolve_tag_commit",
                "timeout",
            ),
            id="operation-timeout",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_IGNORE_TERM": "true",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "resolve_tag_commit",
                "timeout",
            ),
            id="term-ignoring-timeout",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_WORKFLOW_RUN_ID": ""},
                "verify_evidence",
                "missing_evidence",
                enforce=False,
            ),
            id="missing-workflow-run-observation",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_WORKFLOW_FAILURE": "true"},
                "fetch_workflow_run",
                "api_error",
            ),
            id="workflow-run-api-error-enforcement",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_WORKFLOW_FAILURE": "true"},
                "fetch_workflow_run",
                "api_error",
                enforce=False,
            ),
            id="workflow-run-api-error-observation",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "fetch_workflow_run",
                "timeout",
            ),
            id="workflow-run-timeout-enforcement",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "fetch_workflow_run",
                "timeout",
                enforce=False,
            ),
            id="workflow-run-timeout-observation",
        ),
    ],
)
def test_gate_emits_fixed_categories_for_failure_paths(
    tmp_path: Path,
    case: FailureCase,
) -> None:
    """Verify every controlled failure retains a bounded metric category.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    case
        One failure input and its documented fixed category.

    Notes
    -----
    Every failure must emit operation, gate, and workflow-output results before
    the admission script exits unsuccessfully.
    """
    result, metrics, traces, _, outputs = _run_gate(
        tmp_path,
        evidence_state=case.evidence_state,
        extra_environment={
            "NETSUKE_RELEASE_ADMISSION_ENFORCE": str(case.enforce).lower(),
            **case.extra_environment,
        },
    )

    assert result.returncode == (1 if case.enforce else 0), (
        "enforcement must fail closed while observation must retain diagnostics"
    )
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    record = operation_records(metrics, case.operation)[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION[case.operation],
        case.operation,
        "failure",
        case.error_category,
    ), f"{case.operation} must retain its fixed error category"
    assert metrics[-1]["labels"] == expected_gate_labels(
        "failure", case.error_category
    ), "the gate must retain the operation's error category"
    assert outputs["gate-outcome"] == "failure", (
        "failed operations must reach the workflow summary output"
    )
    assert outputs["gate-error-category"] == case.error_category, (
        "failed operations must retain their bounded category in workflow output"
    )
    assert_failure_trace_sequence(traces, case.operation, case.error_category)
    if (
        "NETSUKE_FAKE_WORKFLOW_RUN_ID" in case.extra_environment
        and not case.extra_environment["NETSUKE_FAKE_WORKFLOW_RUN_ID"]
    ):
        workflow_run_record = operation_records(metrics, "fetch_workflow_run")[-1]
        assert workflow_run_record["labels"] == expected_operation_labels(
            CANARY_BY_OPERATION["fetch_workflow_run"],
            "fetch_workflow_run",
            "success",
            "none",
        ), "an empty run identifier must reach evidence verification"
    if case.error_category == "timeout":
        assert operation_duration(metrics, case.operation) > 0, (
            "timed-out operations must retain a positive measured duration"
        )


@given(
    revision=GIT_OBJECT_ID,
    run_id=IDENTIFIER_TEXT,
    path=IDENTIFIER_TEXT,
    url=IDENTIFIER_TEXT,
)
@settings(deadline=None, max_examples=20)
def test_identifiers_never_become_metric_labels(
    revision: str,
    run_id: str,
    path: str,
    url: str,
) -> None:
    """Verify canonical revisions and arbitrary identifiers stay out of labels.

    Parameters
    ----------
    revision
        A canonical 40-character SHA-1 or 64-character SHA-256 object ID.
    run_id, path, url
        Generated unbounded identifiers that must not become labels.

    Notes
    -----
    This property exercises the success path through request construction and
    lookup with canonical Git object IDs. The gate then reports missing
    evidence because the fixture supplies no evidence provider. Malformed
    revisions belong in the failure suite because the SHA-equality check
    rejects them. Run IDs, paths, and URLs remain arbitrary Unicode
    cardinality probes because they do not construct requests.
    """
    identifiers = {
        revision,
        f"run-{run_id}",
        f"path-{path}",
        f"url-{url}",
    }
    with tempfile.TemporaryDirectory() as directory_name:
        result, metrics, traces, calls, outputs = _run_gate(
            Path(directory_name),
            evidence_state="fresh",
            extra_environment={
                "GITHUB_SHA": revision,
                "NETSUKE_RELEASE_ADMISSION_ENFORCE": "false",
                "NETSUKE_FAKE_WORKFLOW_RUN_ID": f"run-{run_id}",
                "NETSUKE_FAKE_PATH": f"path-{path}",
                "NETSUKE_FAKE_URL": f"url-{url}",
            },
        )

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    expected_diagnostics = {
        "path": f"path-{path}",
        "url": f"url-{url}",
    }
    _assert_diagnostics_cross_boundaries(calls, expected_diagnostics)
    _assert_github_requests_cross_boundary(
        calls,
        GITHUB_REPOSITORY,
        revision,
        outputs["gate-error-category"],
    )
    _assert_identifiers_are_excluded(metrics, traces, identifiers)
