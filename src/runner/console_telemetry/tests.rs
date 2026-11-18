//! Verify bounded console-scheduling metrics for actual Ninja invocations.

use super::*;
use crate::runner::NinjaExitError;
use crate::test_tracing_capture::with_test_subscriber;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use monotony::test_util::FixedMonotonicClock;
use rstest::rstest;
use std::io;
use test_support::exit_status;
use tracing_subscriber::filter::LevelFilter;

/// Duration reported by the injected clock for each test invocation.
const BUILD_ELAPSED: Duration = Duration::from_millis(1_250);

/// Represent the local recorder's drained metric series.
type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Find one named metric with exactly the expected bounded labels.
fn metric<'a>(
    snapshot: &'a Snapshot,
    name: &str,
    kind: MetricKind,
    labels: &[(&str, &str)],
) -> Option<&'a DebugValue> {
    snapshot.iter().find_map(|(key, _, _, value)| {
        let actual = key
            .key()
            .labels()
            .map(|label| (label.key(), label.value()))
            .collect::<Vec<_>>();
        (key.kind() == kind
            && key.key().name() == name
            && actual.len() == labels.len()
            && labels.iter().all(|label| actual.contains(label)))
        .then_some(value)
    })
}

/// Assert one bounded operation, one duration sample, and the configured-edge count.
fn assert_metrics(snapshot: &Snapshot, console_edges: usize, labels: [(&str, &str); 3]) {
    assert_eq!(
        snapshot.len(),
        3,
        "only three bounded series are emitted: {snapshot:?}"
    );
    assert!(matches!(
        metric(
            snapshot,
            NINJA_BUILD_EXECUTIONS_TOTAL,
            MetricKind::Counter,
            &labels
        ),
        Some(DebugValue::Counter(1))
    ));
    assert!(
        matches!(metric(snapshot, NINJA_BUILD_EXECUTION_DURATION, MetricKind::Histogram, &labels), Some(DebugValue::Histogram(samples)) if samples.iter().map(|sample| sample.0).eq([BUILD_ELAPSED.as_secs_f64()]))
    );
    let outcome = labels[1].1;
    assert!(
        matches!(metric(snapshot, NINJA_BUILD_CONSOLE_EDGES_TOTAL, MetricKind::Counter, &[("outcome", outcome)]), Some(DebugValue::Counter(count)) if *count == u64::try_from(console_edges).expect("small fixture count"))
    );
}

#[rstest]
#[case::ordinary_success(0, false)]
#[case::console_success(2, false)]
#[case::ordinary_failure(0, true)]
#[case::console_failure(2, true)]
fn invocation_records_bounded_scheduling_outcome_and_duration(
    #[case] console_edges: usize,
    #[case] fail: bool,
) {
    let scheduling = scheduling_label(console_edges);
    let (outcome, category) = if fail {
        ("error", "io")
    } else {
        ("success", "none")
    };
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let clock = FixedMonotonicClock::with_elapsed(BUILD_ELAPSED);
    let (result, events) = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = instrument_ninja_build(&clock, console_edges, || {
                if fail {
                    Err(NinjaInvocationError::io(io::Error::other(
                        "private process detail",
                    )))
                } else {
                    Ok(())
                }
            });
            (result, captured.snapshot())
        })
    });
    assert_eq!(result.is_err(), fail);
    let snapshot = snapshotter.snapshot().into_vec();
    let labels = [
        ("console_scheduling", scheduling),
        ("outcome", outcome),
        ("error_category", category),
    ];
    assert_metrics(&snapshot, console_edges, labels);
    assert!(
        events.iter().any(|event| {
            event.contains(&format!("console_edge_count={console_edges}"))
                && event.contains(&format!("console_scheduling={scheduling:?}"))
                && event.contains(&format!("outcome={outcome:?}"))
                && event.contains(&format!("error_category={category:?}"))
        }),
        "build event must expose the aggregate and fixed outcomes: {events:?}"
    );
    assert!(
        events
            .iter()
            .all(|event| !event.contains("private process detail"))
    );
}

/// A recipe Ninja ran and that failed is not recorded as an I/O failure.
///
/// Both variants surface through the same `io::Error` shape, so only the typed
/// failure can tell a metric consumer which one happened.
#[rstest]
fn unsuccessful_exit_records_a_non_io_error_category() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let clock = FixedMonotonicClock::with_elapsed(BUILD_ELAPSED);
    let (result, events) = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = instrument_ninja_build(&clock, 0, || {
                Err::<(), _>(NinjaInvocationError::exit(
                    NinjaExitError::new(exit_status(7), "build", None)
                        .expect("an unsuccessful status should construct a failure"),
                ))
            });
            (result, captured.snapshot())
        })
    });
    assert!(
        matches!(result, Err(NinjaInvocationError::Exit(_))),
        "the typed exit failure should be returned unchanged"
    );
    let snapshot = snapshotter.snapshot().into_vec();
    assert_metrics(
        &snapshot,
        0,
        [
            ("console_scheduling", "disabled"),
            ("outcome", "error"),
            ("error_category", "exit_status"),
        ],
    );
    assert!(
        events
            .iter()
            .any(|event| event.contains("error_category=\"exit_status\"")),
        "the build event should name the exit-status category: {events:?}"
    );
}
