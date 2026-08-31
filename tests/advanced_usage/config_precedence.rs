//! Configuration-layer precedence and verbose metrics-snapshot coverage.
//!
//! This sibling of `advanced_usage_tests` exercises the parent module's
//! workspace builder with configuration files, CLI arguments, and child-only
//! environment settings. Its assertions prove both precedence behaviour and
//! the bounded metrics snapshot emitted by verbose successful invocations.

use super::{ConfigLayerBuildRequest, run_config_layer_build};
use anyhow::{Result, ensure};
use rstest::rstest;

struct ConfigPrecedenceCase<'a> {
    config_content: &'a str,
    args: &'a [&'a str],
    extra_env: &'a [(&'a str, &'a str)],
    expect_verbose_diagnostics: bool,
}

struct MetricRecordExpectation {
    record: &'static str,
    value: &'static str,
}

struct HistogramRecordExpectation {
    record: &'static str,
}

impl MetricRecordExpectation {
    const fn into_parts(self) -> (&'static str, &'static str) {
        (self.record, self.value)
    }
}

impl HistogramRecordExpectation {
    const fn into_record(self) -> &'static str {
        self.record
    }
}

fn contains_metric_record(snapshot: &str, expected: MetricRecordExpectation) -> bool {
    let (expected_record, expected_value) = expected.into_parts();
    snapshot
        .split("), (")
        .any(|record| record.contains(expected_record) && record.contains(expected_value))
}

fn contains_non_empty_histogram_record(
    snapshot: &str,
    expected: HistogramRecordExpectation,
) -> bool {
    let expected_record = expected.into_record();
    snapshot.split("), (").any(|record| {
        record.contains(expected_record)
            && record.contains("Histogram([")
            && !record.contains("Histogram([])")
    })
}

fn assert_config_metrics_snapshot(stderr: &str) -> Result<()> {
    let snapshot = stderr
        .lines()
        .find(|line| line.contains("metrics snapshot"))
        .ok_or_else(|| anyhow::anyhow!("expected metrics snapshot in stderr: {stderr}"))?;
    let expected_counters = [
        MetricRecordExpectation {
            record: concat!(
                r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
                r#"Label("phase", "diag_mode"), Label("outcome", "success")]"#,
            ),
            value: "Counter(1)",
        },
        MetricRecordExpectation {
            record: concat!(
                r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
                r#"Label("phase", "merge"), Label("outcome", "success")]"#,
            ),
            value: "Counter(1)",
        },
    ];
    let expected_histograms = [
        HistogramRecordExpectation {
            record: concat!(
                r#"CompositeKey(Histogram, Key { name: KeyName("config_load_duration_seconds"), labels: ["#,
                r#"Label("phase", "diag_mode")]"#,
            ),
        },
        HistogramRecordExpectation {
            record: concat!(
                r#"CompositeKey(Histogram, Key { name: KeyName("config_load_duration_seconds"), labels: ["#,
                r#"Label("phase", "merge")]"#,
            ),
        },
    ];
    ensure!(
        snapshot.matches(r#"KeyName("config_load_total")"#).count() == expected_counters.len(),
        "expected exactly two configuration-load counter records: {snapshot}",
    );
    ensure!(
        snapshot
            .matches(r#"KeyName("config_load_duration_seconds")"#)
            .count()
            == expected_histograms.len(),
        "expected exactly two configuration-load duration records: {snapshot}",
    );
    for expected_counter in expected_counters {
        let expected_record = expected_counter.record;
        ensure!(
            contains_metric_record(snapshot, expected_counter),
            "expected counter {expected_record:?} in snapshot: {snapshot}",
        );
    }
    for expected_histogram in expected_histograms {
        let expected_record = expected_histogram.record;
        ensure!(
            contains_non_empty_histogram_record(snapshot, expected_histogram),
            "expected histogram {expected_record:?} in snapshot: {snapshot}",
        );
    }
    Ok(())
}

#[rstest]
#[case::config_file_enables(ConfigPrecedenceCase {
    config_content: "verbose = true\n",
    args: &["build"],
    extra_env: &[],
    expect_verbose_diagnostics: true,
})]
#[case::env_var_enables(ConfigPrecedenceCase {
    config_content: "verbose = false\n",
    args: &["build"],
    extra_env: &[("NETSUKE_VERBOSE", "true")],
    expect_verbose_diagnostics: true,
})]
#[case::env_var_overrides_config(ConfigPrecedenceCase {
    config_content: "verbose = true\n",
    args: &["build"],
    extra_env: &[("NETSUKE_VERBOSE", "false")],
    expect_verbose_diagnostics: false,
})]
#[case::cli_flag_overrides_env(ConfigPrecedenceCase {
    config_content: "verbose = true\n",
    args: &["--verbose", "build"],
    extra_env: &[("NETSUKE_VERBOSE", "false")],
    expect_verbose_diagnostics: true,
})]
fn verbose_config_precedence(#[case] case: ConfigPrecedenceCase<'_>) -> Result<()> {
    let output = run_config_layer_build(ConfigLayerBuildRequest {
        context: "verbose config precedence",
        config_content: case.config_content,
        args: case.args,
        extra_env: case.extra_env,
    })?;
    ensure!(output.success, "expected build to succeed");
    if case.expect_verbose_diagnostics {
        ensure!(
            output.stderr.contains("Timing"),
            "expected verbose timing summary in stderr, got:\n{}",
            output.stderr
        );
        assert_config_metrics_snapshot(&output.stderr)?;
    } else {
        ensure!(
            !output.stderr.contains("Timing"),
            "expected no timing summary, got:\n{}",
            output.stderr
        );
        ensure!(
            !output.stderr.contains("metrics snapshot"),
            "expected no metrics snapshot, got:\n{}",
            output.stderr
        );
    }
    Ok(())
}
