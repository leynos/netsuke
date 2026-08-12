//! Bounded metrics and tracing for attributed command-list failures.

use super::failure_attribution::CommandListFailure;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Duration};

const COMMAND_LIST_FAILURES_TOTAL: &str = "netsuke_ninja_command_list_failures_total";
const COMMAND_LIST_FAILURE_DURATION: &str = "netsuke_ninja_command_list_failure_duration_seconds";

/// Record the only observable per-entry outcome: a safely attributed failure.
pub(super) fn record_failure(failure: &CommandListFailure, elapsed: Duration) {
    describe_metrics();
    tracing::warn!(
        command_list_action = failure.action_identity(),
        command_list_entry = failure.entry_index(),
        command_list_failure = %failure,
        "Ninja command-list entry failed"
    );
    counter!(COMMAND_LIST_FAILURES_TOTAL, "outcome" => "failure").increment(1);
    histogram!(COMMAND_LIST_FAILURE_DURATION, "outcome" => "failure").record(elapsed);
}

fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            COMMAND_LIST_FAILURES_TOTAL,
            "Counts attributed Ninja command-list entry failures."
        );
        describe_histogram!(
            COMMAND_LIST_FAILURE_DURATION,
            "Measures elapsed Ninja build time before an attributed command-list failure."
        );
    });
}

#[cfg(test)]
mod tests {
    //! Metric contracts for bounded command-list failure telemetry.

    use super::*;
    use crate::runner::process::failure_attribution::FailureAttributionWriter;
    use metrics_util::{
        MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use std::io::Write;

    #[test]
    fn attributed_failure_records_bounded_outcome_and_duration() {
        let mut writer = FailureAttributionWriter::new(Vec::new());
        writer
            .write_all(
                concat!(
                    "netsuke command-list failure: action ",
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef, entry 2\n"
                )
                .as_bytes(),
            )
            .expect("marker should parse");
        let failure = writer
            .into_failure()
            .expect("marker should produce attribution");
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            record_failure(&failure, Duration::from_millis(1));
        });
        let snapshot = snapshotter.snapshot().into_vec();
        let has_counter = snapshot.iter().any(|(key, _, _, value)| {
            key.kind() == MetricKind::Counter
                && key.key().name() == COMMAND_LIST_FAILURES_TOTAL
                && matches!(value, DebugValue::Counter(1))
        });
        let has_duration = snapshot.iter().any(|(key, _, _, value)| {
            key.kind() == MetricKind::Histogram
                && key.key().name() == COMMAND_LIST_FAILURE_DURATION
                && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
        });
        assert!(has_counter, "failure counter should record exactly once");
        assert!(has_duration, "failure duration should record one sample");
    }
}
