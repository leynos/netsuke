//! Tests for the bounded fetch metric families.
//!
//! A local `DebuggingRecorder` captures the samples without touching the global
//! recorder, so these cases pin each series and its closed label set in
//! isolation, following the pattern set by the home-resolution counter. The
//! last case drives a real redirecting fetch, so the wiring between the fetch
//! boundary and the emitters is covered rather than the emitters alone.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, bail, ensure};
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use minijinja::value::{Kwargs, Value};
use rstest::rstest;
use test_support::http::{self, HttpResponse};

use super::super::tests_support::{CacheWorkspace, cache_workspace, make_context_with};
use super::super::{NetworkPolicy, fetch};
use super::*;
use crate::stdlib::DEFAULT_FETCH_MAX_RESPONSE_BYTES;

/// One captured metric sample: its name, labels, and value.
struct Sample {
    /// Metric name the sample was recorded under.
    name: String,
    /// Labels attached to the sample, in insertion order.
    labels: Vec<(String, String)>,
    /// Recorded counter or histogram value.
    value: DebugValue,
}

/// Build one label pair for a captured sample.
fn label(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

/// Capture every sample a local recorder observes while running `record`.
fn samples_for(record: impl FnOnce()) -> Vec<Sample> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, record);
    collect_samples(snapshotter.snapshot().into_vec())
}

/// Return the declared failure values a refused redirect may report.
///
/// `none` is excluded: the adapter reserves it for a followed redirect, so a
/// refusal carrying it would contradict the closed vocabulary.
fn refused_failure_values() -> impl Iterator<Item = &'static str> {
    FETCH_REDIRECT_FAILURE_VALUES
        .iter()
        .copied()
        .filter(|failure| *failure != "none")
}

/// Convert raw snapshot entries into samples, keeping the recorder's order.
fn collect_samples(
    entries: Vec<(
        metrics_util::CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )>,
) -> Vec<Sample> {
    entries
        .into_iter()
        .map(|(key, _unit, _description, value)| Sample {
            name: key.key().name().to_owned(),
            labels: key
                .key()
                .labels()
                .map(|pair| (pair.key().to_owned(), pair.value().to_owned()))
                .collect(),
            value,
        })
        .collect()
}

/// Total every counter sample named `name` by its label set.
///
/// The recorder exposes no ordering guarantee, so the totals are keyed by
/// labels and compared as maps.
fn counter_totals(samples: &[Sample], name: &str) -> BTreeMap<Vec<(String, String)>, u64> {
    let mut totals = BTreeMap::new();
    for sample in samples {
        if sample.name != name {
            continue;
        }
        if let DebugValue::Counter(count) = sample.value {
            *totals.entry(sample.labels.clone()).or_insert(0) += count;
        }
    }
    totals
}

/// Assert the counter totals recorded for `name` equal `expected`.
///
/// # Errors
///
/// Returns an error when the recorded totals differ from `expected`.
fn assert_counter_totals(
    samples: &[Sample],
    name: &str,
    expected: &BTreeMap<Vec<(String, String)>, u64>,
) -> Result<()> {
    let recorded = counter_totals(samples, name);
    ensure!(
        &recorded == expected,
        "{name} should record {expected:?}, but recorded {recorded:?}",
    );
    Ok(())
}

/// Return the seconds held by the single label-free fetch duration series.
///
/// # Errors
///
/// Returns an error when the duration series is missing, labelled, not a
/// histogram, or does not hold exactly one observation.
fn fetch_duration_seconds(samples: &[Sample]) -> Result<Vec<f64>> {
    let durations = samples
        .iter()
        .filter(|sample| sample.name == FETCH_DURATION)
        .collect::<Vec<_>>();
    ensure!(
        durations.len() == 1,
        "one fetch must record one duration series, got {}",
        durations.len(),
    );
    let sample = durations
        .first()
        .context("a duration sample must be captured")?;
    ensure!(
        sample.labels.is_empty(),
        "the duration histogram must carry no labels: {:?}",
        sample.labels,
    );
    let DebugValue::Histogram(values) = &sample.value else {
        bail!("a fetch duration must be recorded as a histogram");
    };
    ensure!(
        values.len() == 1,
        "one fetch must record exactly one duration observation, got {}",
        values.len(),
    );
    Ok(values
        .iter()
        .map(|observation| observation.into_inner())
        .collect())
}

/// Assert a redirecting fetch recorded every bounded series.
///
/// # Errors
///
/// Returns an error when a recorded series differs from what one followed
/// redirect produces, or when the duration histogram is missing or holds a
/// non-positive observation.
fn assert_redirected_fetch_metrics(samples: &[Sample]) -> Result<()> {
    assert_counter_totals(
        samples,
        FETCH_TOTAL,
        &BTreeMap::from([(vec![label("outcome", "success")], 1)]),
    )?;
    assert_counter_totals(
        samples,
        FETCH_POLICY_TOTAL,
        &BTreeMap::from([(
            vec![
                label("outcome", "allowed"),
                label("policy_reason", "allowed"),
            ],
            2,
        )]),
    )?;
    assert_counter_totals(
        samples,
        FETCH_REDIRECT_TOTAL,
        &BTreeMap::from([(
            vec![
                label("outcome", "followed"),
                label("redirect_failure", "none"),
            ],
            1,
        )]),
    )?;
    let recorded = fetch_duration_seconds(samples)?;
    ensure!(
        recorded.iter().all(|seconds| *seconds > 0.0),
        "the redirecting fetch must record a positive duration: {recorded:?}",
    );
    Ok(())
}

/// Return the duration the fixture records exactly once.
///
/// A quarter of a second is exactly representable as an IEEE 754 double, so the
/// recorded histogram value can be compared without a tolerance.
const fn sample_duration() -> Duration {
    Duration::from_millis(250)
}

/// Every completed fetch is counted once under its bounded outcome label.
#[rstest]
fn fetch_outcomes_are_counted_under_closed_labels() {
    let samples = samples_for(|| {
        record_fetch(sample_duration(), true);
        record_fetch(Duration::from_millis(500), false);
    });
    assert_eq!(
        counter_totals(&samples, FETCH_TOTAL),
        BTreeMap::from([
            (vec![label("outcome", "success")], 1),
            (vec![label("outcome", "failure")], 1),
        ]),
        "each completed fetch must be counted once under its outcome"
    );
}

/// Every policy decision is counted once under both bounded labels.
#[rstest]
fn policy_decisions_are_counted_under_closed_labels() {
    let samples = samples_for(|| {
        record_policy_decision("allowed", "allowed");
        record_policy_decision("rejected", "host_not_allowlisted");
    });
    assert_eq!(
        counter_totals(&samples, FETCH_POLICY_TOTAL),
        BTreeMap::from([
            (
                vec![
                    label("outcome", "allowed"),
                    label("policy_reason", "allowed"),
                ],
                1,
            ),
            (
                vec![
                    label("outcome", "rejected"),
                    label("policy_reason", "host_not_allowlisted"),
                ],
                1,
            ),
        ]),
        "each policy decision must be counted once under outcome and reason"
    );
}

/// Every declared redirect failure has its own series, and no other does.
#[rstest]
fn redirect_decisions_use_the_declared_failure_vocabulary() {
    let samples = samples_for(|| {
        record_redirect_followed();
        for failure in refused_failure_values() {
            record_redirect_refused(failure);
        }
    });
    let expected = refused_failure_values().fold(
        BTreeMap::from([(
            vec![
                label("outcome", "followed"),
                label("redirect_failure", "none"),
            ],
            1,
        )]),
        |mut totals, failure| {
            *totals
                .entry(vec![
                    label("outcome", "rejected"),
                    label("redirect_failure", failure),
                ])
                .or_insert(0) += 1;
            totals
        },
    );
    assert_eq!(
        counter_totals(&samples, FETCH_REDIRECT_TOTAL),
        expected,
        "each redirect outcome must be counted under its own closed labels"
    );
}

/// A fetch duration is measured under a label-free histogram.
#[rstest]
fn fetch_duration_is_measured_without_labels() {
    let samples = samples_for(|| record_fetch(sample_duration(), true));
    let recorded = fetch_duration_seconds(&samples).expect("the fetch duration should be recorded");
    assert_eq!(
        recorded,
        [sample_duration().as_secs_f64()],
        "one fetch must record exactly its own duration in seconds"
    );
}

/// A redirecting fetch records every bounded series at the fetch boundary.
#[rstest]
fn a_redirected_fetch_records_every_bounded_series(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let (url, requests, server) = match http::spawn_http_server_responses([
        HttpResponse::new(302, "").with_header("Location", "/next"),
        HttpResponse::new(200, "redirected body"),
    ]) {
        Ok(fixture) => fixture,
        // A sandbox that forbids binding a listener cannot host this fixture,
        // and other cases already cover the fetch path it drives.
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!("Skipping fetch metrics test: cannot bind HTTP listener ({err})");
            return Ok(());
        }
        Err(err) => return Err(err).context("spawn redirect fixture for fetch metrics"),
    };
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow HTTP for fetch metrics")?;
    let context = make_context_with(root, policy, DEFAULT_FETCH_MAX_RESPONSE_BYTES);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let fetched =
        metrics::with_local_recorder(&recorder, || fetch(&url, &kwargs, &impure, &context));
    server
        .join()
        .map_err(|err| anyhow::anyhow!("fetch metrics fixture panicked: {err:?}"))?;

    ensure!(
        fetched.is_ok(),
        "the redirecting fetch should succeed: {fetched:?}",
    );
    ensure!(
        requests.load(Ordering::Relaxed) == 2,
        "the fixture should answer both hops",
    );
    let samples = collect_samples(snapshotter.snapshot().into_vec());
    assert_redirected_fetch_metrics(&samples)?;
    Ok(())
}
