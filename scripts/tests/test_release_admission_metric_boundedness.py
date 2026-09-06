"""Exercise the release-admission metric cardinality invariant."""

import tempfile
import typing as typ
from pathlib import Path

from hypothesis import given, settings
from hypothesis import strategies as st
from release_admission_test_support import METRICS_VALIDATOR, _run_gate

IDENTIFIER_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=1,
    max_size=32,
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


def _assert_github_requests_cross_boundary(calls: list[dict[str, object]]) -> None:
    """Verify both bounded GitHub requests reach the fake GitHub boundary."""
    github_arguments = [
        typ.cast("list[str]", call["arguments"])
        for call in calls
        if call["command"] == "gh"
    ]
    assert any(
        any("/commits/" in argument for argument in arguments)
        for arguments in github_arguments
    ), "the commit-resolution request must cross the GitHub boundary"
    assert any(
        any("/actions/runs?" in argument for argument in arguments)
        for arguments in github_arguments
    ), "the workflow-run request must cross the GitHub boundary"


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
    revision=IDENTIFIER_TEXT,
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
    """Verify arbitrary candidate identifiers cannot expand metric cardinality.

    Parameters
    ----------
    revision, run_id, path, url
        Generated unbounded identifiers that must not become labels.

    Notes
    -----
    The generated values remain outside every emitted label dimension.
    """
    identifiers = {
        f"revision-{revision}",
        f"run-{run_id}",
        f"path-{path}",
        f"url-{url}",
    }
    with tempfile.TemporaryDirectory() as directory_name:
        result, metrics, traces, calls, _ = _run_gate(
            Path(directory_name),
            extra_environment={
                "GITHUB_SHA": f"revision-{revision}",
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
    _assert_github_requests_cross_boundary(calls)
    _assert_identifiers_are_excluded(metrics, traces, identifiers)
