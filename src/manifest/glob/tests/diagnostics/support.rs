//! Shared recorder and assertion helpers for glob diagnostics tests.
//!
//! The helpers keep the manifest-template observability cases focused on their
//! outcome contracts while ensuring every recorder and tracing subscriber stays
//! scoped to the individual test.

use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use tracing::level_filters::LevelFilter;

/// Hold the metric-recorder snapshot captured for one test invocation.
pub(super) type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// Name the counter reporting base-cache observations.
pub(super) const BASE_CACHE: &str = "netsuke_manifest_glob_base_cache_total";
/// Name the counter reporting completed glob expansions.
pub(super) const EXPANSIONS: &str = "netsuke_manifest_glob_expansions_total";
/// Name the counter reporting skipped glob entries.
pub(super) const SKIPPED: &str = "netsuke_manifest_glob_entries_skipped_total";
/// Name the counter reporting manifest-template glob results.
pub(super) const TEMPLATE_EXPANSIONS: &str = "netsuke_manifest_template_glob_expansions_total";
/// Name the histogram reporting manifest-template glob duration.
pub(super) const TEMPLATE_EXPANSION_DURATION: &str =
    "netsuke_manifest_template_glob_expansion_duration_seconds";

/// Run `operation` with a local metrics recorder and a capturing subscriber.
pub(super) fn recorded<T>(operation: impl FnOnce() -> T) -> (T, Vec<String>, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (value, events) = metrics::with_local_recorder(&recorder, || {
        crate::test_tracing_capture::with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let value = operation();
            (value, captured.snapshot())
        })
    });
    (value, events, snapshotter.snapshot().into_vec())
}

/// Return a counter value carrying the requested `label`.
pub(super) fn counter_value(snapshot: &Snapshot, name: &str, label: (&str, &str)) -> Option<u64> {
    counter_value_with_labels(snapshot, name, &[label])
}

/// Return a counter value carrying every requested label.
pub(super) fn counter_value_with_labels(
    snapshot: &Snapshot,
    name: &str,
    labels: &[(&str, &str)],
) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, debug_value)| {
        if key.kind() != MetricKind::Counter || key.key().name() != name {
            return None;
        }
        let carries_labels = labels.iter().all(|expected| {
            key.key()
                .labels()
                .any(|found| found.key() == expected.0 && found.value() == expected.1)
        });
        match debug_value {
            DebugValue::Counter(count) if carries_labels => Some(*count),
            _ => None,
        }
    })
}

/// Report whether `snapshot` contains a sample for histogram `name`.
pub(super) fn has_histogram(snapshot: &Snapshot, name: &str) -> bool {
    snapshot.iter().any(|(key, _, _, debug_value)| {
        key.kind() == MetricKind::Histogram
            && key.key().name() == name
            && matches!(debug_value, DebugValue::Histogram(_))
    })
}
