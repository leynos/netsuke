//! Snapshot expectations for configuration-resolution tracing tests.
//!
//! This helper module supplies record-level expectations and assertion
//! functions to its `config_tracing` parent. It keeps snapshot parsing beside
//! the configuration observability contract, while the parent remains
//! responsible for executing the binary and checking human and JSON output.

use anyhow::{Result, ensure};

pub(super) struct ConfigurationDiagnosticOutput<'a> {
    pub(super) stderr: &'a str,
}

#[derive(Clone, Copy)]
pub(super) struct MetricRecordExpectation {
    record: &'static str,
    value: &'static str,
}

pub(super) struct ConfigMetricsExpectation {
    counters: &'static [MetricRecordExpectation],
    histograms: &'static [MetricRecordExpectation],
}

const DIAG_MODE_FAILURE_COUNTER: &str = concat!(
    r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
    r#"Label("phase", "diag_mode"), Label("outcome", "failure")]"#,
);

const DIAG_MODE_DURATION_HISTOGRAM: &str = concat!(
    r#"CompositeKey(Histogram, Key { name: KeyName("config_load_duration_seconds"), labels: ["#,
    r#"Label("phase", "diag_mode")]"#,
);

const DIAG_MODE_SUCCESS_COUNTER: &str = concat!(
    r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
    r#"Label("phase", "diag_mode"), Label("outcome", "success")]"#,
);

const MERGE_SUCCESS_COUNTER: &str = concat!(
    r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
    r#"Label("phase", "merge"), Label("outcome", "success")]"#,
);

const MERGE_FAILURE_COUNTER: &str = concat!(
    r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
    r#"Label("phase", "merge"), Label("outcome", "failure")]"#,
);

const MERGE_DURATION_HISTOGRAM: &str = concat!(
    r#"CompositeKey(Histogram, Key { name: KeyName("config_load_duration_seconds"), labels: ["#,
    r#"Label("phase", "merge")]"#,
);
const CONFIG_LOAD_COUNTER_NAME: &str = r#"KeyName("config_load_total")"#;
const CONFIG_LOAD_DURATION_NAME: &str = r#"KeyName("config_load_duration_seconds")"#;

pub(super) const SUCCESSFUL_EXPLICIT_SELECTION_METRICS: ConfigMetricsExpectation =
    ConfigMetricsExpectation {
        counters: &[
            MetricRecordExpectation {
                record: DIAG_MODE_SUCCESS_COUNTER,
                value: "Counter(1)",
            },
            MetricRecordExpectation {
                record: MERGE_SUCCESS_COUNTER,
                value: "Counter(1)",
            },
        ],
        histograms: &[
            MetricRecordExpectation {
                record: DIAG_MODE_DURATION_HISTOGRAM,
                value: "Histogram([",
            },
            MetricRecordExpectation {
                record: MERGE_DURATION_HISTOGRAM,
                value: "Histogram([",
            },
        ],
    };

pub(super) const DIAGNOSTIC_MODE_FAILURE_METRICS: ConfigMetricsExpectation =
    ConfigMetricsExpectation {
        counters: &[MetricRecordExpectation {
            record: DIAG_MODE_FAILURE_COUNTER,
            value: "Counter(1)",
        }],
        histograms: &[MetricRecordExpectation {
            record: DIAG_MODE_DURATION_HISTOGRAM,
            value: "Histogram([",
        }],
    };

pub(super) const CONFIGURATION_MERGE_FAILURE_METRICS: ConfigMetricsExpectation =
    ConfigMetricsExpectation {
        counters: &[
            MetricRecordExpectation {
                record: DIAG_MODE_SUCCESS_COUNTER,
                value: "Counter(1)",
            },
            MetricRecordExpectation {
                record: MERGE_FAILURE_COUNTER,
                value: "Counter(1)",
            },
        ],
        histograms: &[
            MetricRecordExpectation {
                record: DIAG_MODE_DURATION_HISTOGRAM,
                value: "Histogram([",
            },
            MetricRecordExpectation {
                record: MERGE_DURATION_HISTOGRAM,
                value: "Histogram([",
            },
        ],
    };

fn contains_metric_record(snapshot: &str, expected: MetricRecordExpectation) -> bool {
    snapshot
        .split("), (")
        .any(|record| record.contains(expected.record) && record.contains(expected.value))
}

fn contains_non_empty_histogram_record(snapshot: &str, expected: MetricRecordExpectation) -> bool {
    snapshot.split("), (").any(|record| {
        record.contains(expected.record)
            && record.contains(expected.value)
            && !record.contains("Histogram([])")
    })
}

pub(super) fn assert_config_metrics_snapshot(
    output: &ConfigurationDiagnosticOutput<'_>,
    expected: &ConfigMetricsExpectation,
) -> Result<()> {
    let snapshot = output
        .stderr
        .lines()
        .find(|line| line.contains("metrics snapshot"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "a verbose early exit should emit the configuration metrics snapshot: {}",
                output.stderr
            )
        })?;
    ensure!(
        snapshot.matches(CONFIG_LOAD_COUNTER_NAME).count() == expected.counters.len(),
        "expected exactly {} configuration-load counter records: {snapshot}",
        expected.counters.len(),
    );
    ensure!(
        snapshot.matches(CONFIG_LOAD_DURATION_NAME).count() == expected.histograms.len(),
        "expected exactly {} configuration-load duration records: {snapshot}",
        expected.histograms.len(),
    );
    for expected_counter in expected.counters {
        ensure!(
            contains_metric_record(snapshot, *expected_counter),
            "metrics snapshot should contain counter {:?}: {snapshot}",
            expected_counter.record,
        );
    }
    for expected_histogram in expected.histograms {
        ensure!(
            contains_non_empty_histogram_record(snapshot, *expected_histogram),
            "metrics snapshot should contain histogram {:?}: {snapshot}",
            expected_histogram.record,
        );
    }
    Ok(())
}
