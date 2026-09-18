//! Telemetry coverage for the manifest `env()` lookup boundary.
//!
//! These drive `env_var_with` under a local recorder, so they pin that every
//! lookup outcome — including the blocked refusal the access policy produces —
//! reaches the bounded counter exactly once, and that nothing a manifest
//! supplies reaches a label.

use crate::manifest::{EnvAccessPolicy, EnvReadError, env_reader::env_var_with};
use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;

const ENV_LOOKUP_TOTAL: &str = "netsuke_manifest_env_lookups_total";

/// Stands in for a credential named by a manifest; neither the variable name
/// nor its value may reach a metric label.
const SENTINEL: &str = "s3cr3t-sentinel";
/// Stands in for a credential value that a blocked reader must not return.
const SENTINEL_VALUE: &str = "s3cr3t-value";

type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// Drive one lookup under a local recorder and return its result and snapshot.
///
/// Returning the result rather than discarding it keeps the lookup's own
/// outcome available, so a test can pin the value or error alongside the
/// series it produced.
fn recorded(
    policy: &EnvAccessPolicy,
    read: impl FnOnce() -> Result<String, EnvReadError>,
) -> (Result<String, minijinja::Error>, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let result =
        metrics::with_local_recorder(&recorder, || env_var_with(SENTINEL, policy, |_| read()));
    (result, snapshotter.snapshot().into_vec())
}

/// Value of the environment-lookup counter labelled with `outcome`.
fn lookup_count(snapshot: &Snapshot, outcome: &str) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, value)| {
        if key.kind() != MetricKind::Counter || key.key().name() != ENV_LOOKUP_TOTAL {
            return None;
        }
        let has_outcome = key
            .key()
            .labels()
            .any(|label| label.key() == "outcome" && label.value() == outcome);
        match value {
            DebugValue::Counter(count) if has_outcome => Some(*count),
            _ => None,
        }
    })
}

/// Every retained series must carry exactly the one bounded `outcome` label.
fn every_series_is_bounded(snapshot: &Snapshot) -> bool {
    snapshot.iter().all(|(key, _, _, _)| {
        key.key().labels().count() == 1 && key.key().labels().all(|label| label.key() == "outcome")
    })
}

#[rstest]
#[case::present(false, "success")]
#[case::blocked(true, "blocked")]
#[case::missing(false, "not_present")]
#[case::non_utf8(false, "not_unicode")]
fn each_lookup_outcome_is_counted_once(#[case] blocked: bool, #[case] expected_outcome: &str) {
    let policy = if blocked {
        EnvAccessPolicy::default().block_var(SENTINEL)
    } else {
        EnvAccessPolicy::default()
    };
    let read = || match expected_outcome {
        "success" => Ok(String::from(SENTINEL_VALUE)),
        "not_present" => Err(EnvReadError::NotPresent),
        _ => Err(EnvReadError::NotUnicode),
    };

    let (result, snapshot) = recorded(&policy, read);
    assert_eq!(
        result.as_ref().map(String::as_str).ok(),
        (expected_outcome == "success").then_some(SENTINEL_VALUE),
        "only a permitted, present lookup may return a value: {result:?}"
    );
    assert_eq!(
        lookup_count(&snapshot, expected_outcome),
        Some(1),
        "the lookup must be counted under {expected_outcome}: {snapshot:?}"
    );
    assert_eq!(
        snapshot.len(),
        1,
        "a single lookup must produce a single series: {snapshot:?}"
    );
    assert!(
        every_series_is_bounded(&snapshot),
        "the retained series must carry only the bounded outcome label: {snapshot:?}"
    );
}

/// A blocked lookup is counted before the reader can disclose a value, and the
/// blocked series is the only one the call produces.
#[test]
fn blocked_lookup_increments_only_the_blocked_series() {
    let policy = EnvAccessPolicy::default().block_var(SENTINEL);
    let mut reader_was_called = false;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let error = metrics::with_local_recorder(&recorder, || {
        env_var_with(SENTINEL, &policy, |_| {
            reader_was_called = true;
            Ok(String::from(SENTINEL_VALUE))
        })
        .expect_err("a blocked lookup must refuse")
    });
    let snapshot = snapshotter.snapshot().into_vec();

    assert!(
        !reader_was_called,
        "a blocked lookup must not reach the reader"
    );
    assert!(
        !error.to_string().contains(SENTINEL) && !error.to_string().contains(SENTINEL_VALUE),
        "the refusal must not echo the name or value: {error}"
    );
    assert_eq!(
        lookup_count(&snapshot, "blocked"),
        Some(1),
        "the refusal must be counted: {snapshot:?}"
    );
    assert_eq!(
        snapshot.len(),
        1,
        "a blocked lookup must not also count a read outcome: {snapshot:?}"
    );
    for (key, _, _, _) in &snapshot {
        for label in key.key().labels() {
            assert!(
                !label.value().contains(SENTINEL) && !label.value().contains(SENTINEL_VALUE),
                "neither the variable name nor its value may reach a label: {key:?}"
            );
        }
    }
}
