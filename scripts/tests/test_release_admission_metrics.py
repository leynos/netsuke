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
    FORWARD_COMPATIBLE,
    METRICS_VALIDATOR,
    RELEASE_TAG,
    REVISION,
    SCRIPT_PATH,
    WALL_CLOCK_ARGUMENTS,
    WORKFLOW_RUN_ID,
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

SUCCESSFUL_GATE_OUTPUTS = {
    "gate-outcome": "success",
    "gate-error-category": "none",
    "enforcement": "false",
    "candidate-commit": REVISION,
}
SUCCESSFUL_GATE_RECORD = {
    "name": "netsuke_release_admission_gate_total",
    "labels": expected_gate_labels("success", "none"),
    "value": 1,
}
EXPECTED_TRACE_EVENTS = {
    "operation_complete",
    "gate_complete",
    "workflow_output_delivery",
    "trace_delivery",
}
#: The commit-resolution request, as the gate's own argument vector renders it.
#: A rehearsal names no tag, so the resolution is asked about the revision.
COMMIT_QUERY = ("api", f"repos/leynos/netsuke/commits/{REVISION}", "--jq", ".sha")
#: The workflow-run request, which asks about the run this gate is executing in
#: rather than about whichever run is newest for the revision.
RUN_QUERY = (
    "api",
    f"repos/leynos/netsuke/actions/runs/{WORKFLOW_RUN_ID}",
    "--jq",
    ".head_sha // empty",
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


def _assert_successful_metrics(metrics: list[dict[str, object]]) -> None:
    """Assert the fixed metric contract for a run that admits the candidate."""
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
        assert records[0]["labels"] == expected_operation_labels(
            canary, operation, "success", "none"
        ), f"{operation} must retain its bounded outcome and error category"
    assert metrics[-1] == SUCCESSFUL_GATE_RECORD, (
        "admitting the candidate must leave the gate counter successful"
    )


def _assert_successful_outputs(
    outputs: dict[str, str], tmp_path: pathlib.Path, *, enforce: bool
) -> None:
    """Assert the fixed workflow-output contract for an admitted candidate."""
    expected = {**SUCCESSFUL_GATE_OUTPUTS, "enforcement": str(enforce).lower()}
    assert {name: outputs[name] for name in expected} == expected, (
        "an admitted run must publish its own result and the mode it ran in"
    )
    assert tuple(outputs) == (
        "gate-outcome",
        "gate-error-category",
        "enforcement",
        "candidate-commit",
        "metrics-file",
        "trace-file",
    ), "the workflow must read the six outputs in the documented order"
    assert outputs["metrics-file"] == str(
        tmp_path / "release-admission-metrics.jsonl"
    ), "workflow output must identify the metric artefact"
    assert outputs["trace-file"] == str(tmp_path / "release-admission-traces.jsonl"), (
        "workflow output must identify the trace artefact"
    )


def _assert_successful_traces(traces: list[dict[str, object]]) -> None:
    """Assert the fixed trace sequence for a run that admits the candidate."""
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
            "verify_evidence",
        )
    ] + [
        ("gate_complete", "verify_evidence", "success", "none"),
        ("workflow_output_delivery", "verify_evidence", "success", "none"),
        ("trace_delivery", "verify_evidence", "success", "none"),
    ], "an admitted sequence must retain all bounded trace hand-offs"


def _mode_environment(*, enforce: bool) -> dict[str, str]:
    """Return the child environment that selects one of the gate's two modes.

    A publishing run must name the tag it is admitting, so the enforcing arm
    supplies one; the rehearsal arm names none and is the default.

    Returns
    -------
    dict[str, str]
        The variables that select the requested mode and the tag it needs.
    """
    if not enforce:
        return {"NETSUKE_RELEASE_ADMISSION_ENFORCE": "false"}
    return {
        "NETSUKE_RELEASE_ADMISSION_ENFORCE": "true",
        "NETSUKE_RELEASE_ADMISSION_TAG": RELEASE_TAG,
    }


@pytest.mark.parametrize("mode", ["rehearsal", "publishing"])
def test_gate_admits_the_candidate_on_valid_evidence(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    mode: str,
) -> None:
    """Verify a candidate-bound manifest admits the candidate in either mode.

    Notes
    -----
    Contract invariants: both modes return success, every operation succeeds,
    the native admission requests are unchanged, and all bounded operation,
    gate, and delivery records are written.

    The two modes differ in one request and one output. A rehearsal names no
    tag, so commit resolution is asked about ``GITHUB_SHA`` and the tag itself
    never appears; a publishing run names the release tag, so the same step
    resolves the tag under its ``tags/`` namespace and compares the answer with
    the candidate. The namespace is part of the request because a bare name
    would also match a branch of that name, so the tag's own existence would go
    unchecked. Both must reach the same admission, and the mode the gate
    reports is the one it actually ran in, because publication refuses a
    report-only success.
    """
    enforce = mode == "publishing"
    subject = f"tags/{RELEASE_TAG}" if enforce else REVISION
    commit_query = ("api", f"repos/leynos/netsuke/commits/{subject}", "--jq", ".sha")
    boundaries = install_boundaries(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment=_mode_environment(enforce=enforce),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert boundaries.gh.calls == [list(commit_query), list(RUN_QUERY)], (
        "the GitHub adapter must receive both native admission queries"
    )
    assert boundaries.git.calls == [list(FETCH_ARGUMENTS)], (
        "the Git adapter must receive the bounded native fetch arguments"
    )
    assert boundaries.clock.calls == [], (
        "the default clock is the real interpreter, not the unused double"
    )
    _assert_successful_metrics(run.metrics)
    _assert_successful_outputs(run.outputs, tmp_path, enforce=enforce)
    _assert_successful_traces(run.traces)


def test_an_unrequired_result_does_not_refuse_a_valid_manifest(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a manifest reporting a future canary still publishes.

    Notes
    -----
    Issue #598 adds downstream canaries to the same manifest. A gate that
    refused a document carrying a result it does not require would make every
    such addition a breaking change across the two workflows, so the extra
    result is ignored rather than fatal -- and this is asserted in the
    publishing mode, where refusing would block the release.
    """
    install_boundaries(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=FORWARD_COMPATIBLE,
        extra_environment=_mode_environment(enforce=True),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    _assert_successful_outputs(run.outputs, tmp_path, enforce=True)


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
    assert {tuple(call) for call in adapted.clock.calls} == {
        CLOCK_ARGUMENTS,
        WALL_CLOCK_ARGUMENTS,
    }, "the clock adapter must receive both clock program argument vectors"
    assert len(adapted.clock.calls) == 12, (
        "the gate reads the clock twice per operation, once for freshness, and "
        "once at the admission decision"
    )
    assert adapted.clock.calls.count(list(WALL_CLOCK_ARGUMENTS)) == 2, (
        "the window must be read at the freshness check and again at the decision"
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
