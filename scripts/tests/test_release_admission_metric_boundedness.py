"""Exercise the release-admission metric cardinality invariant."""

import tempfile
import typing as typ
from pathlib import Path

from hypothesis import given, settings
from hypothesis import strategies as st
from release_admission_test_support import (
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    _run_gate,
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
