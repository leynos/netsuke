//! Telemetry assertions for the user-scope pass.
//!
//! The user pass adds an observable outcome: a dangling user candidate fails
//! discovery instead of silently loading system configuration. These tests run
//! the real discovery path and record its outcome through the composition
//! boundary, so the signal they check is the one an operator's collector sees
//! rather than one built from a synthetic error.

use super::{Case, HomeArrangement};
use crate::cli::DISCOVERY_TOTAL;
use crate::cli::discovery::{DiscoveryOutcome, record_discovery_outcome};
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use monotony::MonotonicClock;
use rstest::rstest;
use tracing_subscriber::filter::LevelFilter;

/// One recorded metric sample from the debugging recorder.
type Sample = (
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
);

/// Record `outcome` under an isolated recorder, returning its samples and the
/// bounded discovery-span fields.
fn record(outcome: &DiscoveryOutcome) -> (Vec<Sample>, Vec<String>) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let clock = monotony::StdMonotonicClock;
    let started = clock.now();
    let fields = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::TRACE, |captured| {
            record_discovery_outcome(&clock, started, outcome);
            captured.discovery_span_fields()
        })
    });
    (snapshotter.snapshot().into_vec(), fields)
}

/// Return how many samples are discovery counters carrying `outcome`.
///
/// The counter's documented label set is `outcome` alone; the finer
/// `error_category` is a span field. Keeping the two apart here means a change
/// that widens the metric's cardinality fails this test rather than passing
/// unnoticed.
fn counters_with_outcome(samples: &[Sample], outcome: &str) -> usize {
    samples
        .iter()
        .filter(|(key, _, _, value)| {
            key.kind() == MetricKind::Counter
                && key.key().name() == DISCOVERY_TOTAL
                && key
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(value, DebugValue::Counter(1))
        })
        .count()
}

/// A dangling user candidate records a bounded discovery-error signal.
///
/// The outcome is `OrthoError::File` rather than a new error kind, so the
/// decision is visible through the existing discovery series and span. The
/// test first requires discovery to fail: if the candidate were treated as
/// absent, discovery would succeed with the system layer and the assertions
/// below would be checking a success signal instead.
#[cfg(unix)]
#[rstest]
fn dangling_user_candidate_records_a_bounded_error_signal() -> Result<()> {
    let case = Case::new()?;
    let link = case.home().join(".netsuke.toml");
    // The target is relative so the link stays inside the home directory,
    // which is the shape `ortho_config` reports as an absent file.
    test_support::fs::symlink("absent-target.toml", &link)
        .context("create dangling home symlink")?;

    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);
    let outcome = case.discover_outcome(&environment);
    let error = outcome
        .first_error()
        .context("a dangling user candidate should fail discovery")?;
    ensure!(
        matches!(**error, ortho_config::OrthoError::File { .. }),
        "the fail-closed error should be a file error so telemetry can classify it: {error:?}"
    );

    let (samples, fields) = record(&outcome);

    ensure!(
        counters_with_outcome(&samples, "error") == 1,
        "the failed pass should be counted exactly once as an error"
    );
    ensure!(
        counters_with_outcome(&samples, "success") == 0,
        "a fail-closed pass must not also be counted as a success"
    );
    ensure!(
        fields.contains(&"outcome=\"error\"".to_owned()),
        "the discovery span should record the failed outcome: {fields:?}"
    );
    ensure!(
        fields.contains(&"error_category=\"file\"".to_owned()),
        "the discovery span should record the bounded category: {fields:?}"
    );
    Ok(())
}
