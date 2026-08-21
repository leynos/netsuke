//! Bounded observability for configuration file-layer discovery.
//!
//! Discovery is a query that reads the environment and filesystem once and
//! returns loaded layers plus deferred diagnostics. The span, counter, and
//! duration histogram live here so the pure discovery flow in
//! [`super::discovery`] stays readable and every emitted field stays bounded:
//! only the outcome and a coarse error category ever reach a subscriber.
//!
//! Discovery defers its granular diagnostics to the composition boundary
//! (see [`super::trace::DiscoveryDiagnostics`]); this module records only the
//! coarse pass outcome, never selectors, paths, or error text.

use super::DiscoveryOutcome;
use crate::cli::{DISCOVERY_DURATION, DISCOVERY_TOTAL};
use metrics::{counter, describe_counter, describe_histogram, histogram};
use monotony::MonotonicClock;
use ortho_config::OrthoError;
use std::sync::Once;
use std::time::Instant;
use tracing::field;

/// Register the discovery metric descriptions exactly once per process.
pub(super) fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            DISCOVERY_TOTAL,
            "Counts configuration discovery passes by bounded outcome."
        );
        describe_histogram!(
            DISCOVERY_DURATION,
            "Measures configuration discovery duration in seconds."
        );
    });
}

/// Classify a discovery failure into a closed label set.
///
/// `OrthoError` is `#[non_exhaustive]`, so unknown future variants map to a
/// stable `other` label rather than leaking error text into telemetry.
const fn error_category(error: &OrthoError) -> &'static str {
    match error {
        OrthoError::File { .. } => "file",
        OrthoError::Validation { .. } => "validation",
        OrthoError::CyclicExtends { .. } => "cyclic_extends",
        OrthoError::CliParsing(_) => "cli_parsing",
        OrthoError::Gathering(_) => "gathering",
        OrthoError::Merge { .. } => "merge",
        OrthoError::Aggregate(_) => "aggregate",
        _ => "other",
    }
}

/// Run `discover` inside a bounded discovery span at the composition boundary.
///
/// Discovery is a query; instrumentation belongs at the composition boundary
/// where a monotonic clock is available, never inside the query. The span and
/// metrics carry only the pass outcome and, on failure, a coarse error
/// category. Selectors, paths, and configuration values never become
/// telemetry, matching the path-safe field policy of the deferred diagnostics.
pub(super) fn timed_discovery<C: MonotonicClock>(
    clock: &C,
    discover: impl FnOnce() -> DiscoveryOutcome,
) -> DiscoveryOutcome {
    describe_metrics();
    let span = tracing::trace_span!(
        "collect_diag_file_layers",
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = clock.now();
    let outcome = discover();
    let first_error = outcome.first_error().cloned();
    if let Some(error) = first_error.as_deref() {
        let category = error_category(error);
        span.record("outcome", "error");
        span.record("error_category", category);
        tracing::debug!(error_category = category, "configuration discovery failed");
        counter!(DISCOVERY_TOTAL, "outcome" => "error").increment(1);
    } else {
        span.record("outcome", "success");
        counter!(DISCOVERY_TOTAL, "outcome" => "success").increment(1);
    }
    histogram!(DISCOVERY_DURATION).record(clock.now().duration_since(started));
    outcome
}

/// Record the discovery outcome series at the composition boundary.
///
/// This is for boundaries that already timed the phase (for example the
/// startup diagnostic resolution) and only need the discovery counter plus
/// error-category span from the retained outcome.
pub fn record_discovery_outcome<C: MonotonicClock>(
    clock: &C,
    started: Instant,
    outcome: &DiscoveryOutcome,
) {
    describe_metrics();
    histogram!(DISCOVERY_DURATION).record(clock.now().duration_since(started));
    if let Some(error) = outcome.first_error() {
        let category = error_category(error);
        tracing::debug!(error_category = category, "configuration discovery failed");
        counter!(DISCOVERY_TOTAL, "outcome" => "error").increment(1);
    } else {
        counter!(DISCOVERY_TOTAL, "outcome" => "success").increment(1);
    }
}

#[cfg(test)]
mod tests {
    //! Tests for bounded discovery telemetry.

    use super::*;
    use crate::cli::discovery::{DiscoveredLayers, DiscoveryDiagnostics, DiscoveryOutcome};
    use metrics_util::{
        CompositeKey, MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use std::sync::Arc;

    type Snapshot = Vec<(
        CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )>;

    /// Assert that one discovery counter has the expected bounded outcome.
    fn assert_discovery_counter(snapshot: &Snapshot, expected_outcome: &str) {
        let matches = snapshot
            .iter()
            .filter(|(key, _, _, value)| {
                key.kind() == MetricKind::Counter
                    && key.key().name() == DISCOVERY_TOTAL
                    && key
                        .key()
                        .labels()
                        .any(|label| label.key() == "outcome" && label.value() == expected_outcome)
                    && matches!(value, DebugValue::Counter(1))
            })
            .count();
        assert_eq!(
            matches, 1,
            "discovery counter should record {expected_outcome:?} exactly once"
        );
    }

    /// Assert that one discovery-duration histogram contains one sample.
    fn assert_discovery_duration(snapshot: &Snapshot) {
        let matches = snapshot
            .iter()
            .filter(|(key, _, _, value)| {
                key.kind() == MetricKind::Histogram
                    && key.key().name() == DISCOVERY_DURATION
                    && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
            })
            .count();
        assert_eq!(matches, 1, "duration histogram should record one sample");
    }

    /// Run one timed discovery under an isolated recorder and return its snapshot.
    fn snapshot_timed_discovery(discover: impl FnOnce() -> DiscoveryOutcome) -> Snapshot {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let clock = monotony::StdMonotonicClock;
        metrics::with_local_recorder(&recorder, || {
            drop(timed_discovery(&clock, discover));
        });
        snapshotter.snapshot().into_vec()
    }

    /// A discovery outcome with no errors and no pending load warnings.
    fn outcome_without_error() -> DiscoveryOutcome {
        let (layers, errors, diagnostics) = empty_parts();
        DiscoveryOutcome {
            layers: DiscoveredLayers {
                layers,
                json_preference: false,
                errors,
                diagnostics,
            },
        }
    }

    /// A discovery outcome carrying a bounded file-load error.
    fn outcome_with_error() -> DiscoveryOutcome {
        let error = Arc::new(ortho_config::OrthoError::File {
            path: "/nonexistent/netsuke-telemetry.toml".into(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "missing config",
            )),
        });
        let (layers, _errors, diagnostics) = empty_parts();
        DiscoveryOutcome {
            layers: DiscoveredLayers {
                layers,
                json_preference: false,
                errors: vec![error],
                diagnostics,
            },
        }
    }

    /// Build the empty layer vector, error vector, and deferred diagnostics.
    fn empty_parts() -> (
        Vec<ortho_config::MergeLayer<'static>>,
        Vec<Arc<ortho_config::OrthoError>>,
        DiscoveryDiagnostics,
    ) {
        let resolution = super::super::ConfigPathResolution {
            selector: "none",
            path: None,
            environment_lookups: Vec::new(),
        };
        let diagnostics = DiscoveryDiagnostics::new(
            super::super::trace::DiscoveryTrace::new(
                &resolution,
                super::super::trace::FileLayerTrace::Automatic {
                    project_scope: None,
                },
            ),
            None,
        );
        (Vec::new(), Vec::new(), diagnostics)
    }

    #[test]
    fn error_category_is_closed_and_bounded() {
        let outcome = outcome_with_error();
        let error = outcome.first_error().expect("error outcome has an error");
        let category = error_category(error);
        assert!(
            matches!(
                category,
                "file"
                    | "validation"
                    | "cyclic_extends"
                    | "cli_parsing"
                    | "gathering"
                    | "merge"
                    | "aggregate"
                    | "other"
            ),
            "category must be from the closed set, got {category}"
        );
    }

    #[test]
    fn instrument_discovery_records_success_counter_and_duration() {
        let snapshot = snapshot_timed_discovery(outcome_without_error);

        assert_discovery_counter(&snapshot, "success");
        assert_discovery_duration(&snapshot);
    }

    #[test]
    fn instrument_discovery_records_error_counter() {
        let snapshot = snapshot_timed_discovery(outcome_with_error);

        assert_discovery_counter(&snapshot, "error");
    }
}
