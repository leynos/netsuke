//! Shared assertions for configuration-observability metric snapshots.
//!
//! Integration-test binaries use this fixture to verify the rendered
//! `DebuggingRecorder` snapshot contract without duplicating record parsing.

use anyhow::{Result, ensure};

/// One expected configuration metric record in a rendered snapshot.
#[derive(Clone, Copy)]
pub struct MetricSnapshotRecord {
    /// Stable metric name.
    pub name: &'static str,
    /// Complete expected label rendering, in snapshot format.
    pub labels: &'static [&'static str],
    /// Expected value when the metric has a stable rendered value.
    pub value: Option<&'static str>,
}

/// Assert every expected configuration metric record appears in `stderr`.
///
/// Each check is scoped to one `CompositeKey` record, requires exactly the
/// expected label cardinality, and leaves histogram samples unconstrained.
///
/// # Errors
///
/// Returns an error when the snapshot marker is absent or any expected record
/// is absent or does not match.
pub fn assert_config_metrics_snapshot(
    stderr: &str,
    expected_records: &[MetricSnapshotRecord],
) -> Result<()> {
    ensure!(
        stderr.contains("metrics snapshot"),
        "expected a configuration metrics snapshot in stderr: {stderr}"
    );
    for expected in expected_records {
        let metric_name = format!("name: KeyName(\"{}\")", expected.name);
        ensure!(
            stderr.split("CompositeKey(").skip(1).any(|record| {
                record.contains(&metric_name)
                    && expected.labels.iter().all(|label| record.contains(label))
                    && record.matches("Label(").count() == expected.labels.len()
                    && expected.value.is_none_or(|value| record.contains(value))
            }),
            "expected configuration metric record {} with labels {:?} in stderr: {stderr}",
            expected.name,
            expected.labels,
        );
    }
    Ok(())
}
