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

/// The recorded failure categories a Ninja invocation can report.
///
/// Each variant owns both the failure a case records and the assertions that
/// distinguish it, so a future failure class adds one variant instead of
/// another copy of the recorder, clock and snapshot scaffolding.
#[derive(Clone, Copy, Debug)]
enum RecordedFailure {
    /// The selected program could not be started.
    Launch,
    /// Ninja started and reported an unsuccessful status.
    ExitStatus,
    /// Another process input/output fault.
    Io,
}

impl RecordedFailure {
    /// Build the typed invocation failure this category describes.
    fn construct(self) -> NinjaInvocationError {
        match self {
            Self::Launch => NinjaInvocationError::spawn(
                camino::Utf8Path::new("ninja"),
                io::Error::new(io::ErrorKind::NotFound, "private process detail"),
            ),
            Self::ExitStatus => NinjaInvocationError::exit(
                NinjaExitError::new(exit_status(7), "build", None)
                    .expect("an unsuccessful status should construct a failure"),
            ),
            Self::Io => NinjaInvocationError::io(io::Error::other("private process detail")),
        }
    }

    /// Name the bounded `error_category` label this failure records.
    fn label(self) -> &'static str {
        match self {
            Self::Launch => "launch",
            Self::ExitStatus => "exit_status",
            Self::Io => "io",
        }
    }

    /// Report whether the returned error kept this category's variant.
    ///
    /// A label check alone would pass a classifier that renamed every variant
    /// to the expected category, so the variant itself must survive.
    fn is_preserved_in(self, error: &NinjaInvocationError) -> bool {
        match self {
            Self::Launch => matches!(error, NinjaInvocationError::Spawn { .. }),
            Self::ExitStatus => matches!(error, NinjaInvocationError::Exit(_)),
            Self::Io => matches!(error, NinjaInvocationError::Io(_)),
        }
    }
}

/// Each typed failure records its own bounded category and survives unchanged.
///
/// All three variants reach the metric consumer through the same error shape,
/// so only the typed failure can say which one happened. The snapshot is
/// matched by its exact label set, which is what rejects a wrong category.
#[rstest]
#[case::launch(RecordedFailure::Launch)]
#[case::exit_status(RecordedFailure::ExitStatus)]
#[case::io(RecordedFailure::Io)]
fn failure_records_its_error_category(#[case] failure: RecordedFailure) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let clock = FixedMonotonicClock::with_elapsed(BUILD_ELAPSED);
    let (result, events) = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = instrument_ninja_build(&clock, 0, || Err::<(), _>(failure.construct()));
            (result, captured.snapshot())
        })
    });
    let error = result.expect_err("the typed failure should be returned unchanged");
    assert!(
        failure.is_preserved_in(&error),
        "the {} failure should keep its variant: {error:?}",
        failure.label()
    );
    let labels = [
        ("console_scheduling", "disabled"),
        ("outcome", "error"),
        ("error_category", failure.label()),
    ];
    assert_metrics(&snapshotter.snapshot().into_vec(), 0, labels);
    let expected = format!("error_category={:?}", failure.label());
    assert!(
        events.iter().any(|event| event.contains(&expected)),
        "the build event should name the {} category: {events:?}",
        failure.label()
    );
}
