r"""Exercise bounded release-admission metrics on the observed happy paths.

Every case here drives the real gate through cmd-mox doubles, so what is under
assertion is the production boundary: the native argument vectors the gate
builds, the records it writes, and the exit status the workflow step reads.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_metrics.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import stat
import typing as typ

import pytest
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    CLOCK_ARGUMENTS,
    METRICS_VALIDATOR,
    REVISION,
    SCRIPT_PATH,
    AdaptedBoundaries,
    expected_gate_labels,
    expected_operation_labels,
    install_adapted,
    install_boundaries,
    operation_records,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

FRESH_OBSERVATION_GATE_OUTPUTS = {
    "gate-outcome": "failure",
    "gate-error-category": "missing_evidence",
}
FRESH_OBSERVATION_GATE_RECORD = {
    "name": "netsuke_release_admission_gate_total",
    "labels": expected_gate_labels("failure", "missing_evidence"),
    "value": 1,
}
EXPECTED_TRACE_EVENTS = {
    "operation_complete",
    "gate_complete",
    "workflow_output_delivery",
    "trace_delivery",
}
#: The commit-resolution request, as the gate's own argument vector renders it.
COMMIT_QUERY = ("api", f"repos/leynos/netsuke/commits/{REVISION}", "--jq", ".sha")
#: The workflow-run request, which asks for the run covering the revision.
RUN_QUERY = (
    "api",
    f"repos/leynos/netsuke/actions/runs?head_sha={REVISION}&per_page=1",
    "--jq",
    ".workflow_runs[0].id // empty",
)
#: The bounded fetch, with the separator that stops a revision being an option.
FETCH_ARGUMENTS = ("fetch", "--depth", "1", "--no-tags", "origin", "--", REVISION)
#: The PEP 723 requirements the workflow's direct invocation depends on: the
#: Python baseline the estate pins, and the two libraries the gate imports.
REQUIRED_PEP723_PINS = (
    'requires-python = ">=3.14"',
    '"cuprum==0.1.0",',
    '"cyclopts==4.25.3",',
)


def _trace_signature(trace: dict[str, object]) -> tuple[object, ...]:
    """Return the bounded fields that identify one trace record."""
    return tuple(
        trace[field] for field in ("event", "operation", "outcome", "error_category")
    )


def _assert_fresh_observation_metrics(metrics: list[dict[str, object]]) -> None:
    """Assert the fixed metric contract for synthetic fresh evidence."""
    assert len(metrics) == 11, "five operations need counters and durations plus gate"
    duration_operations = {
        record["labels"]["operation"]
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_duration_seconds"
        and isinstance(record["labels"], dict)
    }
    assert duration_operations == CANARY_BY_OPERATION.keys(), (
        "every fixed operation must emit its bounded duration"
    )
    for operation, canary in CANARY_BY_OPERATION.items():
        records = operation_records(metrics, operation)
        assert len(records) == 1, f"{operation} must emit exactly one counter"
        outcome = "failure" if operation == "verify_evidence" else "success"
        error_category = "missing_evidence" if outcome == "failure" else "none"
        assert records[0]["labels"] == expected_operation_labels(
            canary, operation, outcome, error_category
        ), f"{operation} must retain its bounded outcome and error category"
    assert metrics[-1] == FRESH_OBSERVATION_GATE_RECORD, (
        "synthetic freshness must retain the producer-backed evidence failure"
    )


def _assert_fresh_observation_outputs(
    outputs: dict[str, str], tmp_path: pathlib.Path
) -> None:
    """Assert the fixed workflow-output contract for synthetic fresh evidence."""
    assert {
        name: outputs[name] for name in FRESH_OBSERVATION_GATE_OUTPUTS
    } == FRESH_OBSERVATION_GATE_OUTPUTS, "observation must publish the gate result"
    assert tuple(outputs) == (
        "gate-outcome",
        "gate-error-category",
        "metrics-file",
        "trace-file",
    ), "the workflow must read the four outputs in the documented order"
    assert outputs["metrics-file"] == str(
        tmp_path / "release-admission-metrics.jsonl"
    ), "workflow output must identify the metric artefact"
    assert outputs["trace-file"] == str(tmp_path / "release-admission-traces.jsonl"), (
        "workflow output must identify the trace artefact"
    )


def _assert_fresh_observation_traces(traces: list[dict[str, object]]) -> None:
    """Assert the fixed trace sequence for synthetic fresh evidence."""
    assert {trace["event"] for trace in traces} == EXPECTED_TRACE_EVENTS, (
        "traces must include operation, gate, output, and delivery boundaries"
    )
    assert [_trace_signature(trace) for trace in traces] == [
        ("operation_complete", operation, "success", "none")
        for operation in (
            "resolve_tag_commit",
            "fetch_candidate_revision",
            "fetch_workflow_run",
            "check_scan_freshness",
        )
    ] + [
        (
            "operation_complete",
            "verify_evidence",
            "failure",
            "missing_evidence",
        ),
        ("gate_complete", "verify_evidence", "failure", "missing_evidence"),
        ("workflow_output_delivery", "verify_evidence", "success", "none"),
        ("trace_delivery", "verify_evidence", "success", "none"),
    ], "successful operation sequence must retain all bounded trace hand-offs"


def test_gate_observes_synthetic_fresh_evidence_without_blocking(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify synthetic fresh evidence remains an observed gate failure.

    Notes
    -----
    Contract invariants: observation mode returns success while retaining the
    failed gate result, the native admission requests, and all bounded
    operation and trace records.
    """
    boundaries = install_boundaries(cmd_mox)
    run = run_gate(cmd_mox, tmp_path, evidence_state="fresh")

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert boundaries.gh.calls == [list(COMMIT_QUERY), list(RUN_QUERY)], (
        "the GitHub adapter must receive both native admission queries"
    )
    assert boundaries.git.calls == [list(FETCH_ARGUMENTS)], (
        "the Git adapter must receive the bounded native fetch arguments"
    )
    assert boundaries.clock.calls == [], (
        "the default clock is the real interpreter, not the unused double"
    )
    _assert_fresh_observation_metrics(run.metrics)
    _assert_fresh_observation_outputs(run.outputs, tmp_path)
    _assert_fresh_observation_traces(run.traces)


def test_documented_adapters_receive_native_boundary_contracts(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify every documented adapter receives only its native contract.

    Notes
    -----
    Contract invariants: each adapter variable names an executable directly;
    the API and Git adapters receive native command arguments, the clock
    receives the Python program argument, and each sink receives its configured
    file target with one record on standard input.
    """
    adapted = install_adapted(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment=adapted.variables(cmd_mox),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert adapted.gh.calls == [list(COMMIT_QUERY), list(RUN_QUERY)], (
        "a redirected GitHub adapter must receive both native admission queries"
    )
    assert adapted.git.calls == [list(FETCH_ARGUMENTS)], (
        "a redirected Git adapter must receive the bounded native fetch arguments"
    )
    assert {tuple(call) for call in adapted.clock.calls} == {CLOCK_ARGUMENTS}, (
        "the clock adapter must receive the Python clock program arguments"
    )
    assert len(adapted.clock.calls) == 10, (
        "the gate reads the clock once before and once after each operation"
    )
    assert len(adapted.metrics.calls) == len(run.metrics), (
        "the metric sink must receive exactly the records the artefact holds"
    )
    _assert_sink_adapter_targets(adapted, run.paths)
    assert adapted.metrics.stdins == _rendered_lines(run.paths["metrics"]), (
        "the metric sink must receive each rendered record on standard input"
    )
    assert adapted.output.stdins == _rendered_lines(run.paths["output"]), (
        "the output sink must receive each workflow-output line on standard input"
    )
    assert adapted.trace.stdins == _rendered_lines(run.paths["trace"]), (
        "the trace sink must receive each rendered record on standard input"
    )


def _rendered_lines(path: pathlib.Path) -> list[str]:
    """Return one artefact's records, each terminated as the gate wrote it."""
    return [
        f"{line}\n" for line in path.read_text(encoding="utf-8").splitlines() if line
    ]


def _assert_sink_adapter_targets(
    adapted: AdaptedBoundaries, paths: dict[str, pathlib.Path]
) -> None:
    """Assert each sink double was invoked with only its own target."""
    expected = {
        "metrics": paths["metrics"],
        "output": paths["output"],
        "trace": paths["trace"],
    }
    for name, target in expected.items():
        calls = getattr(adapted, name).calls
        assert calls, f"{name} sink must receive records"
        assert set(map(tuple, calls)) == {(str(target),)}, (
            f"{name} sink must receive only its configured output target"
        )


def test_validator_rejects_non_finite_metric_values() -> None:
    """Verify the JSON contract rejects non-finite metric observations.

    Notes
    -----
    Contract invariant: JSON ``Infinity`` is rejected before metric schema
    validation can accept the record.
    """
    with pytest.raises(ValueError, match="release-admission metric records"):
        METRICS_VALIDATOR.parse_metrics(['{"value": Infinity}'])


def test_entry_point_declares_the_runtime_contract_the_workflow_needs() -> None:
    """Verify the entry point is a runnable, self-describing ``uv`` script.

    Notes
    -----
    Contract invariants: the shebang is the estate's ``uv`` form, the PEP 723
    block pins the Python baseline and both dependencies, and the file keeps
    its executable bit so a direct invocation works.
    """
    text = SCRIPT_PATH.read_text(encoding="utf-8")
    lines = text.splitlines()
    assert lines[0] == "#!/usr/bin/env -S uv run --script", (
        "the entry point must keep the estate's uv shebang"
    )
    block = _pep723_block(lines)
    missing = [pin for pin in REQUIRED_PEP723_PINS if pin not in block]
    assert not missing, (
        f"the PEP 723 block must declare every runtime requirement; missing {missing}"
    )
    mode = SCRIPT_PATH.stat().st_mode
    assert mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH) == (
        stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
    ), "the entry point must stay executable for the workflow"


def _pep723_block(lines: list[str]) -> str:
    """Return the PEP 723 metadata block, as one newline-joined string."""
    opener = lines.index("# /// script")
    closer = lines.index("# ///", opener + 1)
    return "\n".join(lines[opener : closer + 1])
