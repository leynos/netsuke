"""Protect publisher phase reporting and bounded retry diagnostics."""

import json
from pathlib import Path

import pytest
from release_operation_catalog import OPERATION_PHASES, publisher_context
from release_operation_observability import (
    observation_from_environment,
    render_observation,
)
from release_operation_observability_support import (
    DRY_RUN_INPUT,
    assert_retained_telemetry_policy,
    assert_summary_environment,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    step_index_by_key,
)


@pytest.fixture(scope="module")
def release_workflow() -> dict[str, object]:
    """Load release.yml for publisher telemetry assertions."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def test_crates_io_job_retains_phase_observability(
    release_workflow: dict[str, object],
) -> None:
    """Retain phase outputs in an always-run summary and JSONL artifact."""
    steps = job_steps(release_workflow, "publish-crates-io")
    action = named_step(steps, "Publish and verify the crates.io version")
    assert action.get("id") == "publish_crates_io", "identify the action outputs"
    env = assert_summary_environment(
        steps, "publish_crates_io", DRY_RUN_INPUT, staged=False
    )
    assert env.get("PUBLISHER_OUTPUTS") == (
        "${{ toJSON(steps.publish_crates_io.outputs) }}"
    ), "the reporter must consume only the action's declared outputs"
    assert step_index_by_key(steps, "id", "publish_crates_io") < step_index_by_key(
        steps, "name", "Record release operation observability"
    ), "the reporter must run after the publisher action"
    assert_retained_telemetry_policy(steps)


@pytest.fixture(scope="module")
def publisher_telemetry() -> tuple[str, str, list[dict[str, object]]]:
    """Build the publisher telemetry fixture shared by focused contract tests."""
    outputs = {
        f"{phase.replace('_', '-')}-{suffix}": value
        for phase in OPERATION_PHASES["publish_crates_io"]
        for suffix, value in (
            ("outcome", "success"),
            ("started-ns", "1000000000"),
            ("ended-ns", "2000000000"),
        )
    }
    outputs.update({
        "crate-name": "netsuke-build-1.2.3-beta.1.crate",
        "package-name": "netsuke-build",
        "crate-version": "1.2.3-beta.1",
        "registry-state": "absent",
        "action": "publish",
        "registry-download-attempts": "3",
        "registry-download-status-class": "4xx",
        "registry-download-elapsed-ms": "20123",
        "registry-byte-verification-outcome": "success",
        "sha256": "candidate-digest-not-for-telemetry",
        "token": "secret-token-not-for-telemetry",
        "response-body": "raw-response-not-for-telemetry",
        "url": "https://example.invalid/not-for-telemetry",
    })
    observation = observation_from_environment(
        {
            "RELEASE_OPERATION": "publish_crates_io",
            "RELEASE_DRY_RUN": "false",
            "JOB_STATUS": "success",
            "OPERATION_STARTED_NS": "900000000",
            "UPLOAD_ERROR_PRESENT": "false",
            "PUBLISHER_OUTPUTS": json.dumps(outputs),
        },
        ended_ns=2_100_000_000,
    )

    summary, jsonl = render_observation(observation)
    records = [json.loads(line) for line in jsonl.splitlines()]
    return summary, jsonl, records


def test_publisher_summary_retains_bounded_context(
    publisher_telemetry: tuple[str, str, list[dict[str, object]]],
) -> None:
    """Keep the approved publisher summary and validated context fields."""
    summary, _, records = publisher_telemetry
    expected_summary = (
        Path(__file__).parent
        / "snapshots"
        / "release_publish_crates_io_observability_summary.snapshot"
    ).read_text(encoding="utf-8")
    assert summary == expected_summary, (
        "publisher summary must retain bounded registry context and retry facts"
    )
    trace_records = [
        record for record in records if record.get("record_type") == "trace_event"
    ]
    assert trace_records, "publisher telemetry must emit phase trace records"
    assert all(
        record.get("crate_name") == "netsuke-build"
        and record.get("version") == "1.2.3-beta.1"
        and record.get("registry_state") == "absent"
        and record.get("action") == "publish"
        and record.get("verification_verdict") == "success"
        for record in trace_records
    ), "trace events must retain the bounded publisher context"


def test_publisher_retry_metrics_retain_bounded_values(
    publisher_telemetry: tuple[str, str, list[dict[str, object]]],
) -> None:
    """Record retry count, status class, and elapsed time as fixed metrics."""
    _, _, records = publisher_telemetry
    retry_metrics = {
        record["metric"]: record
        for record in records
        if record.get("record_type") == "metric"
        and str(record.get("metric", "")).startswith("release_registry_download_")
    }
    assert retry_metrics["release_registry_download_attempts"]["value"] == 3, (
        "retry metrics must retain the bounded download attempt count"
    )
    assert retry_metrics["release_registry_download_elapsed_ms"]["value"] == 20123, (
        "retry metrics must retain the elapsed download time"
    )
    assert (
        retry_metrics["release_registry_download_status_class"]["status_class"] == "4xx"
    ), "retry metrics must retain only the HTTP status class"


def test_publisher_telemetry_omits_raw_sensitive_values(
    publisher_telemetry: tuple[str, str, list[dict[str, object]]],
) -> None:
    """Exclude digest, token, response body, and URL values from telemetry."""
    summary, jsonl, _ = publisher_telemetry
    assert not any(
        value in summary + jsonl
        for value in (
            "candidate-digest-not-for-telemetry",
            "secret-token-not-for-telemetry",
            "raw-response-not-for-telemetry",
            "https://example.invalid/not-for-telemetry",
        )
    ), "telemetry must omit digests, tokens, bodies, and URLs"


def test_publisher_context_discards_invalid_registry_labels() -> None:
    """Keep unsafe identifiers and unrecognized state labels out of telemetry."""
    context = publisher_context({
        "package-name": "netsuke-build\nInjected heading",
        "crate-version": "1.0.0`malicious",
        "registry-state": [],
        "action": {"publish": True},
        "registry-byte-verification-outcome": "unexpected",
    })

    assert context == {"verification_verdict": "unknown"}, (
        "publisher telemetry must accept only valid identifiers and fixed labels"
    )


@pytest.mark.parametrize("registry_state", ["conflict", "yanked", "indeterminate"])
def test_rejected_registry_decisions_remain_in_failure_telemetry(
    registry_state: str,
) -> None:
    """Retain rejected state and action in the failed publisher record."""
    outputs = {
        "crate-name": "netsuke-build-1.2.3.crate",
        "package-name": "netsuke-build",
        "crate-version": "1.2.3",
        "registry-state": registry_state,
        "action": "fail",
        "registry-decision-outcome": "failure",
        "registry-decision-started-ns": "1000000000",
        "registry-decision-ended-ns": "2000000000",
        "registry-byte-verification-outcome": "skipped",
    }
    observation = observation_from_environment(
        {
            "RELEASE_OPERATION": "publish_crates_io",
            "RELEASE_DRY_RUN": "false",
            "JOB_STATUS": "failure",
            "UPLOAD_ERROR_PRESENT": "false",
            "PUBLISHER_OUTPUTS": json.dumps(outputs),
        },
        ended_ns=2_100_000_000,
    )

    summary, jsonl = render_observation(observation)
    records = [json.loads(line) for line in jsonl.splitlines()]
    trace_records = [
        record for record in records if record.get("record_type") == "trace_event"
    ]

    assert f"`registry_state`: `{registry_state}`" in summary, (
        "report the rejected registry observation"
    )
    assert "`action`: `fail`" in summary, "report that classification blocked mutation"
    assert trace_records, "emit a phase record for the failed classifier"
    assert all(
        record.get("registry_state") == registry_state
        and record.get("action") == "fail"
        for record in trace_records
    ), "trace records must retain rejected decisions"
