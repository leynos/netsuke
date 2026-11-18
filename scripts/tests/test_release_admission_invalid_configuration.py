"""Prove a refused release-admission configuration stops before any operation.

Each case supplies an out-of-contract configuration value and asserts on what
the production boundary saw: the gate fails closed, emits only its own failure
metric, publishes the fixed ``unknown`` category, and never reaches the GitHub,
Git, or clock adapters. The runtime and failure-path metrics live in
``test_release_admission_metric_failures``.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_invalid_configuration.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

import pytest
from release_admission_test_support import (
    CLOCK_ARGUMENTS,
    METRICS_VALIDATOR,
    RELEASE_TAG,
    AdaptedBoundaries,
    install_adapted,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

#: A refused configuration emits exactly one metric: the gate's own.
INVALID_CONFIGURATION_METRICS = [
    {
        "name": "netsuke_release_admission_gate_total",
        "labels": {"outcome": "failure", "error_category": "unknown"},
        "value": 1,
    }
]
#: Its traces stop after the gate record, because no operation ever ran.
INVALID_CONFIGURATION_TRACE_SIGNATURES = [
    ("gate_complete", "verify_evidence", "failure", "unknown"),
    ("workflow_output_delivery", "verify_evidence", "success", "none"),
    ("trace_delivery", "verify_evidence", "success", "none"),
]


def _trace_signature(trace: dict[str, object]) -> tuple[object, ...]:
    """Return the bounded fields that identify one trace record."""
    return tuple(
        trace[field] for field in ("event", "operation", "outcome", "error_category")
    )


def _assert_unreachable_boundaries(
    adapted: AdaptedBoundaries, expected_clock_calls: int
) -> None:
    """Assert no admission operation ran, but the clock still reached its double."""
    assert adapted.gh.calls == [], (
        "a refused configuration must not reach the GitHub adapter"
    )
    assert adapted.git.calls == [], (
        "a refused configuration must not reach the Git adapter"
    )
    assert len(adapted.clock.calls) == expected_clock_calls, (
        "a refused configuration must not read the clock for any operation"
    )
    assert {tuple(call) for call in adapted.clock.calls} <= {CLOCK_ARGUMENTS}, (
        "every clock call must carry the Python monotonic program"
    )


@pytest.mark.parametrize(
    ("setting", "value"),
    [
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS",
            "0",
            id="timeout-seconds-0",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS",
            "301",
            id="timeout-seconds-301",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS",
            "not-a-number",
            id="timeout-seconds-not-a-number",
        ),
        pytest.param("NETSUKE_RELEASE_ADMISSION_ENFORCE", "", id="enforce-empty"),
        pytest.param("NETSUKE_RELEASE_ADMISSION_ENFORCE", "False", id="enforce-False"),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_ENFORCE", "observe", id="enforce-observe"
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS",
            "0",
            id="max-age-zero",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS",
            "604801",
            id="max-age-past-the-week",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS",
            "0604800",
            id="max-age-leading-zero",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS",
            "1000000",
            id="max-age-too-many-digits",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS",
            "٨٦٤٠٠",
            id="max-age-non-ascii-digits",
        ),
    ],
)
def test_invalid_configuration_fails_before_running_admission_operations(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    setting: str,
    value: str,
) -> None:
    """Verify invalid configuration fails before any external command runs.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    setting
        Closed configuration input that receives an invalid value.
    value
        Out-of-contract configuration value supplied to the gate.

    Notes
    -----
    Early validation must still emit valid gate metrics, traces, and workflow
    outputs while preventing every external admission operation. The clock
    adapter is redirected at a double here, so its absence is observed rather
    than assumed from a name the gate would never resolve anyway.
    """
    adapted = install_adapted(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment={**adapted.variables(cmd_mox), setting: value},
    )

    assert run.result.returncode != 0, "invalid configuration must fail closed"
    _assert_unreachable_boundaries(adapted, expected_clock_calls=0)
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.metrics == INVALID_CONFIGURATION_METRICS, (
        "invalid configuration must emit only the fixed failure gate metric"
    )
    assert run.outputs["gate-outcome"] == "failure", (
        "invalid configuration must publish failure"
    )
    assert run.outputs["gate-error-category"] == "unknown", (
        "invalid configuration must publish the fixed unknown category"
    )
    assert [_trace_signature(trace) for trace in run.traces] == (
        INVALID_CONFIGURATION_TRACE_SIGNATURES
    ), "early configuration failure must retain the bounded trace hand-off sequence"


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("0", id="zero"),
        pytest.param("604801", id="past-the-week"),
    ],
)
def test_an_invalid_evidence_age_fails_an_enforcing_run_before_any_operation(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    value: str,
) -> None:
    """Verify an enforcing run refuses an out-of-contract evidence bound.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    value
        Out-of-contract maximum evidence age supplied to the gate.

    Notes
    -----
    The bound is what keeps a stale manifest out of an enforcing run, so a
    value outside the contract must stop the run before any admission
    operation. ``0`` is not a bound at all, and one second past a week is past
    the largest bound the gate accepts; neither may be read as "unbounded".
    Enforcement is set explicitly so the refusal cannot be an observation-mode
    success.
    """
    adapted = install_adapted(cmd_mox)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment={
            **adapted.variables(cmd_mox),
            "NETSUKE_RELEASE_ADMISSION_ENFORCE": "true",
            # An enforcing run must name the tag it admits, or configuration
            # resolution refuses it before the bound is ever consulted.
            "NETSUKE_RELEASE_ADMISSION_TAG": RELEASE_TAG,
            "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS": value,
        },
    )

    assert run.result.returncode == 1, "an invalid evidence bound must exit 1"
    _assert_unreachable_boundaries(adapted, expected_clock_calls=0)
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.metrics == INVALID_CONFIGURATION_METRICS, (
        "an invalid evidence bound must emit only the fixed failure gate metric"
    )
    assert run.outputs["gate-outcome"] == "failure", (
        "an invalid evidence bound must publish failure"
    )
    assert run.outputs["gate-error-category"] == "unknown", (
        "an invalid evidence bound must publish the fixed unknown category"
    )
