//! Telemetry assertions for the user-scope pass.
//!
//! The user pass adds an observable outcome: a dangling user candidate fails
//! discovery instead of silently loading system configuration. These tests run
//! the real discovery path and record its outcome through the composition
//! boundary, so the signal they check is the one an operator's collector sees
//! rather than one built from a synthetic error.

use super::{Case, HomeArrangement};
use crate::cli::DISCOVERY_TOTAL;
use crate::cli::discovery::event_assertions::{EventAssertion, capture_events};
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

/// Event text marking a user-scope chain as the one discovery adopted.
const ADOPTED_MESSAGE: &str = "adopted a user candidate";

/// Event text marking discovery as having delegated to the dependency scan.
const FALLBACK_MESSAGE: &str = "used the dependency scan";

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

/// Report whether `sample` is the discovery counter for exactly `outcome`.
///
/// The counter's documented label set is `outcome` alone; the finer
/// `error_category` is a span field. Requiring the complete label set, rather
/// than merely finding a matching `outcome` label among others, means a change
/// that widens the metric's cardinality fails this test instead of passing
/// unnoticed.
fn is_discovery_counter(sample: &Sample, outcome: &str) -> bool {
    let (key, _, _, value) = sample;
    key.kind() == MetricKind::Counter
        && key.key().name() == DISCOVERY_TOTAL
        && key.key().labels().count() == 1
        && key
            .key()
            .labels()
            .any(|label| label.key() == "outcome" && label.value() == outcome)
        && matches!(value, DebugValue::Counter(1))
}

/// Return every sample that is a discovery counter, whatever its labels.
///
/// Filtering by name and kind before looking at labels is what makes a widened
/// series observable: a second sample carrying extra labels, or a second
/// outcome series, is counted here even though [`is_discovery_counter`] would
/// reject it.
fn discovery_counters(samples: &[Sample]) -> Vec<&Sample> {
    samples
        .iter()
        .filter(|(key, _, _, _)| {
            key.kind() == MetricKind::Counter && key.key().name() == DISCOVERY_TOTAL
        })
        .collect()
}

/// Build a sample named as the discovery counter, labelled as `labels` says.
fn discovery_sample(labels: Vec<metrics::Label>) -> Sample {
    let key = CompositeKey::new(
        MetricKind::Counter,
        metrics::Key::from_parts(DISCOVERY_TOTAL, labels),
    );
    (key, None, None, DebugValue::Counter(1))
}

/// The label-set check accepts the documented series and rejects a wider one.
///
/// The helper is the only thing standing between a widened label set and a
/// passing test, so both directions are exercised: without the accepting case a
/// helper that rejected everything would pass the rejection case alone, and
/// without the rejecting case the cardinality guarantee would be asserted but
/// never tested.
#[rstest]
fn discovery_counter_label_set_is_exact() {
    let documented = discovery_sample(vec![metrics::Label::new("outcome", "error")]);
    assert!(
        is_discovery_counter(&documented, "error"),
        "a counter labelled only by outcome must satisfy the documented label set"
    );

    let widened = discovery_sample(vec![
        metrics::Label::new("outcome", "error"),
        metrics::Label::new("error_category", "dangling_link"),
    ]);
    assert!(
        !is_discovery_counter(&widened, "error"),
        "a counter carrying an extra label must not satisfy the documented label set"
    );
}

/// A dangling user candidate records a bounded discovery-error signal.
///
/// The outcome is `OrthoError::File` rather than a new error kind, so the
/// decision is visible through the existing discovery series and span. The
/// test first requires discovery to fail: if the candidate were treated as
/// absent, discovery would succeed with the system layer and the assertions
/// below would be checking a success signal instead.
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

    // Exactly one discovery counter may exist, and it must be the documented
    // error series with value 1. Counting the error series alone would accept a
    // second sample that carried extra labels or the other outcome.
    let counters = discovery_counters(&samples);
    ensure!(
        counters.len() == 1,
        "the failed pass should record exactly one discovery counter, got {}: {counters:?}",
        counters.len()
    );
    let counter = counters
        .first()
        .copied()
        .context("exactly one counter is present when the count is one")?;
    ensure!(
        is_discovery_counter(counter, "error"),
        "the only discovery counter must carry just outcome=\"error\" with value 1, got {counter:?}"
    );
    ensure!(
        fields.contains(&"outcome=\"error\"".to_owned()),
        "the discovery span should record the failed outcome: {fields:?}"
    );
    ensure!(
        fields.contains(&"error_category=\"dangling_link\"".to_owned()),
        "the discovery span should name the fail-closed branch, not the generic file category: {fields:?}"
    );
    Ok(())
}

/// Discover and replay in separate captured phases.
///
/// One capture cannot distinguish replay from emission during discovery: a
/// regression that logged where the decision is made would pass on the replay
/// call alone. In production discovery runs before the composition boundary
/// raises the filter, so an early event is dropped; requiring the discovery
/// phase to carry no scope decision is what makes these tests sensitive to that
/// contract. The replay phase is captured on its own so its events cannot be
/// confused with anything discovery emitted.
fn discover_without_scope_decision(
    discover: impl FnOnce() -> DiscoveryOutcome,
) -> Result<(DiscoveryOutcome, Vec<String>)> {
    let (outcome, discovery_events) = capture_events(|| Ok::<_, anyhow::Error>(discover()))?;
    for message in [ADOPTED_MESSAGE, FALLBACK_MESSAGE] {
        ensure!(
            !discovery_events.iter().any(|line| line.contains(message)),
            "discovery emitted {message:?} before the tracing filter was raised, \
             so a verbose run would drop it: {discovery_events:?}"
        );
    }
    let ((), replay_events) = capture_events(|| {
        outcome.emit_diagnostics();
        Ok::<_, anyhow::Error>(())
    })?;
    Ok((outcome, replay_events))
}

/// Require the replay to carry `message` exactly once and return that event.
///
/// Counting rather than finding keeps a doubly emitted decision, or one emitted
/// by two code paths, from satisfying a case that only needs it to appear.
fn exactly_one_event<'a>(events: &'a [String], message: &str) -> Result<&'a String> {
    let matches: Vec<&String> = events
        .iter()
        .filter(|line| line.contains(message))
        .collect();
    ensure!(
        matches.len() == 1,
        "the replay should emit {message:?} exactly once, got {}: {events:?}",
        matches.len()
    );
    matches
        .first()
        .copied()
        .context("a single match is present when the count is one")
}

/// Adopting a user candidate records the scope and skips the dependency scan.
///
/// The event is what distinguishes "the user's file won" from "we fell back",
/// so it must name the scope, the skipped pass, and a correlation hash without
/// carrying the candidate path. It is emitted on replay rather than where the
/// decision is made, so the case asks for the replay under verbose tracing, and
/// the fallback event must be absent — otherwise the two branches would not be
/// distinguishable.
#[rstest]
fn adopted_user_candidate_records_scope_without_a_path() -> Result<()> {
    let case = Case::new()?;
    let home_config = case.write_home_dotfile("jobs = 4\n")?;
    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);

    let (outcome, events) =
        discover_without_scope_decision(|| case.discover_outcome(&environment))?;
    ensure!(
        outcome.first_error().is_none(),
        "the user file should load without error"
    );

    let event = exactly_one_event(&events, ADOPTED_MESSAGE)?;
    ensure!(
        event.contains("scope=\"user\""),
        "adoption should name the user scope: {event}"
    );
    ensure!(
        event.contains("skipped_compose_layers=true"),
        "adoption should record that the dependency scan was skipped: {event}"
    );
    ensure!(
        !events.iter().any(|line| line.contains(FALLBACK_MESSAGE)),
        "an adopted user file must not also report the fallback: {events:?}"
    );
    EventAssertion::new(event, &home_config).ensure_bounded_path_fields()?;
    Ok(())
}

/// Falling back to the dependency scan records the other scope and no path.
///
/// A case with no user file exercises the fallback branch. The event must not
/// carry any path or path hash: the dependency scan reads several locations and
/// naming one of them would misreport which file supplied policy. It is
/// replayed under verbose tracing, and the adoption event must be absent so the
/// two branches stay distinguishable.
#[rstest]
fn fallback_to_the_dependency_scan_records_scope_without_a_path() -> Result<()> {
    let case = Case::new()?;
    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);

    let (outcome, events) =
        discover_without_scope_decision(|| case.discover_outcome(&environment))?;
    ensure!(
        outcome.first_error().is_none(),
        "the system file should load without error"
    );

    let event = exactly_one_event(&events, FALLBACK_MESSAGE)?;
    ensure!(
        event.contains("scope=\"system_or_project\""),
        "fallback should name the scope it delegated to: {event}"
    );
    ensure!(
        event.contains("skipped_compose_layers=false"),
        "fallback should record that the dependency scan ran: {event}"
    );
    // Naming the exact fields is not enough: a raw path could travel in a field
    // this test does not know about, so the system file the scan read is also
    // checked for absence by path and by file name.
    ensure!(
        !event.contains("path_hash") && !event.contains("path_present"),
        "the fallback event must not name a configuration path: {event}"
    );
    EventAssertion::new(event, &case.system_config()).ensure_raw_path_absent()?;
    ensure!(
        !events.iter().any(|line| line.contains(ADOPTED_MESSAGE)),
        "a fallback must not also report an adopted user file: {events:?}"
    );
    Ok(())
}
