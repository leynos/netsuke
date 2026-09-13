//! Tests for the bounded fetch metric families.
//!
//! A local `DebuggingRecorder` captures the samples without touching the global
//! recorder, so these cases pin each series and its closed label set in
//! isolation, following the pattern set by the home-resolution counter. The
//! last two cases drive a real redirecting fetch, followed and refused, so the
//! wiring between the fetch boundary and the emitters is covered rather than
//! the emitters alone.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use metrics_util::debugging::DebuggingRecorder;
use minijinja::value::{Kwargs, Value};
use rstest::rstest;
use test_support::http::{self, HttpResponse, HttpServer};

use super::super::tests_support::{
    CacheWorkspace, Sample, assert_counter_totals, cache_workspace, collect_samples,
    counter_totals, fetch_duration_seconds, label, make_context_with,
};
use super::super::{FetchContext, NetworkPolicy, fetch};
use super::*;
use crate::stdlib::DEFAULT_FETCH_MAX_RESPONSE_BYTES;

/// Return whether the sandbox refused to bind the fixture's listener.
///
/// A denied bind says nothing about the fetch path under test, so those cases
/// skip; every other spawn failure is reported with its caller's context.
fn bind_permission_denied(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::PermissionDenied
}

/// Capture every sample a local recorder observes while running `record`.
fn samples_for(record: impl FnOnce()) -> Vec<Sample> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, record);
    collect_samples(snapshotter.snapshot().into_vec())
}

/// Drive one fetch under a local recorder and return its outcome and samples.
///
/// The recorder is local rather than global, so the samples describe exactly
/// this fetch, following the pattern the home-resolution counter set. The
/// default request carries no keyword arguments and no purity flag of its own.
fn fetch_recording_samples(
    url: &str,
    context: &FetchContext,
) -> (Result<Value, minijinja::Error>, Vec<Sample>) {
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let fetched = metrics::with_local_recorder(&recorder, || fetch(url, &kwargs, &impure, context));
    (fetched, collect_samples(snapshotter.snapshot().into_vec()))
}

/// Join the fixture's server, naming the fixture in any panic report.
///
/// # Errors
///
/// Returns an error when the fixture thread panicked.
fn join_fixture(server: HttpServer, label: &str) -> Result<()> {
    server
        .join()
        .map_err(|err| anyhow::anyhow!("{label} fixture panicked: {err:?}"))
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

/// Expected policy totals for one allowed hop and one rejected redirect.
fn allowed_then_rejected_policy_totals() -> BTreeMap<Vec<(String, String)>, u64> {
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
    ])
}

/// Expected redirect totals for one followed redirect.
fn followed_redirect_total() -> BTreeMap<Vec<(String, String)>, u64> {
    BTreeMap::from([(
        vec![
            label("outcome", "followed"),
            label("redirect_failure", "none"),
        ],
        1,
    )])
}

/// Assert a fetch recorded exactly one positive duration observation.
///
/// # Errors
///
/// Returns an error when the duration series is missing, labelled, not a
/// histogram, does not hold exactly one observation, or holds a non-positive
/// one.
fn assert_positive_duration(samples: &[Sample], fetch: &str) -> Result<()> {
    let recorded = fetch_duration_seconds(samples)?;
    ensure!(
        recorded.iter().all(|seconds| *seconds > 0.0),
        "the {fetch} fetch must record a positive duration: {recorded:?}",
    );
    Ok(())
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
    assert_counter_totals(samples, FETCH_REDIRECT_TOTAL, &followed_redirect_total())?;
    assert_positive_duration(samples, "redirecting")
}

/// Assert a refused redirect recorded every bounded refusal series.
///
/// The followed case pins the success path; this is the other half of the
/// contract, and the only path that reaches the rejected and failure labels
/// through `fetch`.
///
/// # Errors
///
/// Returns an error when a recorded series differs from what one refused
/// redirect produces, or when the duration histogram is missing or holds a
/// non-positive observation.
fn assert_refused_redirect_metrics(samples: &[Sample]) -> Result<()> {
    assert_counter_totals(
        samples,
        FETCH_TOTAL,
        &BTreeMap::from([(vec![label("outcome", "failure")], 1)]),
    )?;
    assert_counter_totals(
        samples,
        FETCH_POLICY_TOTAL,
        &allowed_then_rejected_policy_totals(),
    )?;
    assert_counter_totals(
        samples,
        FETCH_REDIRECT_TOTAL,
        &BTreeMap::from([(
            vec![
                label("outcome", "rejected"),
                label("redirect_failure", "policy_rejected"),
            ],
            1,
        )]),
    )?;
    assert_positive_duration(samples, "refused")
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
        allowed_then_rejected_policy_totals(),
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
    let expected =
        refused_failure_values().fold(followed_redirect_total(), |mut totals, failure| {
            *totals
                .entry(vec![
                    label("outcome", "rejected"),
                    label("redirect_failure", failure),
                ])
                .or_insert(0) += 1;
            totals
        });
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

    let (fetched, samples) = fetch_recording_samples(&url, &context);
    join_fixture(server, "fetch metrics")?;

    ensure!(
        fetched.is_ok(),
        "the redirecting fetch should succeed: {fetched:?}",
    );
    ensure!(
        requests.load(Ordering::Relaxed) == 2,
        "the fixture should answer both hops",
    );
    assert_redirected_fetch_metrics(&samples)?;
    Ok(())
}

/// A refused redirect records its bounded refusal series at the fetch boundary.
///
/// The fixture answers one request; its redirect names a host the policy does
/// not allow, so the refusal happens before a second request is dispatched and
/// the failure counters are what the fetch boundary reports.
#[rstest]
fn a_refused_redirect_records_its_bounded_series(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let (url, requests, server) =
        match http::spawn_http_server_responses([
            HttpResponse::new(302, "").with_header("Location", "http://blocked.example/next")
        ]) {
            Ok(fixture) => fixture,
            // A sandbox that forbids binding a listener cannot host this fixture,
            // and other cases already cover the fetch path it drives.
            Err(err) if bind_permission_denied(&err) => {
                tracing::warn!("Skipping refused fetch metrics test: cannot bind listener ({err})");
                return Ok(());
            }
            Err(err) => {
                return Err(err).context("spawn refused redirect fixture for fetch metrics");
            }
        };
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow HTTP for fetch metrics")?
        .deny_all_hosts()
        .allow_hosts(["127.0.0.1"])
        .context("allow the fixture host for fetch metrics")?;
    let context = make_context_with(root, policy, DEFAULT_FETCH_MAX_RESPONSE_BYTES);

    let (fetched, samples) = fetch_recording_samples(&url, &context);
    join_fixture(server, "refused fetch metrics")?;

    ensure!(
        fetched.is_err(),
        "a redirect to a host outside the allowlist must be refused: {fetched:?}",
    );
    ensure!(
        requests.load(Ordering::Relaxed) == 1,
        "a refused redirect must not reach its target, got {} requests",
        requests.load(Ordering::Relaxed),
    );
    assert_refused_redirect_metrics(&samples)?;
    Ok(())
}
