//! Shared fixtures and assertion helpers for the network fetch tests.
//!
//! Provides the temporary cache workspace fixture, `FetchContext` builders,
//! captured metric samples, and reusable assertions for cache-directory,
//! policy, and bounded-series checks.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use metrics_util::debugging::DebugValue;
use minijinja::{
    ErrorKind,
    value::{Kwargs, Value},
};
use rstest::fixture;
use tempfile::tempdir;
use test_support::fs;
use url::Url;

use super::telemetry::FETCH_DURATION;
use super::{FetchContext, NetworkConfig, NetworkPolicy, fetch, open_cache_dir};
use crate::localization;
use crate::stdlib::{DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES};

/// Username the network tests place in a fixture URL's userinfo.
pub(super) const REDIRECT_USER: &str = "redirect-user";
/// Password the network tests place in a fixture URL's userinfo.
pub(super) const REDIRECT_SECRET: &str = "redirect-secret";

pub(super) type CacheWorkspace = (tempfile::TempDir, Arc<Dir>, Utf8PathBuf);

/// Creates a temporary cache workspace returning the tempdir, an ambient
/// authority directory handle wrapped in `Arc`, and the UTF-8 path for cache
/// assertions in fetch tests.
#[fixture]
pub(super) fn cache_workspace() -> Result<CacheWorkspace> {
    let temp = tempdir().context("create fetch cache tempdir")?;
    let temp_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("tempdir path not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(temp_path.as_path(), ambient_authority())
        .context("open cache workspace")?;
    Ok((temp, Arc::new(dir), temp_path))
}

/// Build a credentialed URL for the loopback fixture at `fixture_url`.
///
/// Every network case is driven against a local fixture, and each one needs the
/// hop to carry userinfo so the redaction contract is exercised. Deriving the
/// credentialed URL from the fixture's own URL keeps the host and the ephemeral
/// port in one place: a helper that formats `127.0.0.1:{port}` itself has to
/// track whatever the fixture actually bound, and drifts from it silently.
///
/// The credentials are the same pair `SECRETS` names, so a diagnostic that
/// discloses either is caught wherever this helper is used.
///
/// # Errors
///
/// Returns an error when `fixture_url` is not a well-formed URL or does not name
/// a host.
pub(super) fn credentialed_loopback_url(fixture_url: &str) -> Result<Url> {
    let mut url = Url::parse(fixture_url)
        .with_context(|| format!("fixture URL should parse: {fixture_url}"))?;
    url.set_username(REDIRECT_USER)
        .map_err(|()| anyhow!("fixture URL should accept a username: {fixture_url}"))?;
    url.set_password(Some(REDIRECT_SECRET))
        .map_err(|()| anyhow!("fixture URL should accept a password: {fixture_url}"))?;
    url.set_path("/start");
    Ok(url)
}

/// Builds a test `FetchContext` with the provided cache root and default policy.
pub(super) fn make_context(root: Arc<Dir>) -> FetchContext {
    make_context_with(
        root,
        NetworkPolicy::default(),
        DEFAULT_FETCH_MAX_RESPONSE_BYTES,
    )
}

pub(super) fn make_context_with(root: Arc<Dir>, policy: NetworkPolicy, limit: u64) -> FetchContext {
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy,
        max_response_bytes: limit,
    };
    FetchContext::new(config)
}

/// Computes `limit + offset` as a `usize` for oversized-response fixtures.
pub(super) fn limit_with_offset(limit: u64, offset: u64) -> Result<usize> {
    let total = limit
        .checked_add(offset)
        .context("test limit plus offset should not overflow")?;
    usize::try_from(total).context("test limit plus offset should fit into usize")
}

/// Write an entry to the cache directory and assert it exists within the workspace.
pub(super) fn assert_cache_entry_exists(
    dir: Dir,
    cache_relative: &Utf8Path,
    workspace: &Utf8Path,
    entry_name: &str,
) -> Result<()> {
    dir.write(entry_name, b"data")
        .context("write cache entry")?;
    drop(dir);
    let entry = workspace.join(cache_relative).join(entry_name);
    ensure!(
        fs::exists(entry.as_std_path()),
        "entry {entry} should exist"
    );
    Ok(())
}

/// Asserts that `open_cache_dir` rejects the `path` with an error message containing `expected`.
pub(super) fn assert_open_cache_dir_rejects(
    root: &Dir,
    path: &Utf8Path,
    expected: &str,
) -> Result<()> {
    let err = open_cache_dir(root, path).expect_err("open_cache_dir should reject invalid path");
    ensure!(
        err.to_string().contains(expected),
        "error should mention {expected}, got {err}",
    );
    Ok(())
}

/// Asserts that `fetch` rejects `url` under `policy` without marking the template impure.
pub(super) fn assert_fetch_policy_rejection(
    root: Arc<Dir>,
    policy: NetworkPolicy,
    url: &str,
    expected_message: &str,
) -> Result<()> {
    let context = make_context_with(root, policy, DEFAULT_FETCH_MAX_RESPONSE_BYTES);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch(url, &kwargs, &impure, &context) else {
        return Err(anyhow!("expected fetch to reject '{url}'"));
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "fetch should report InvalidOperation on policy rejection but was {:?}",
        err.kind(),
    );
    ensure!(
        err.to_string().contains(expected_message),
        "error should mention expected message '{expected_message}': {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "policy rejection must not mark the template impure",
    );
    Ok(())
}

pub(super) fn cache_relative_error(key: &'static str, path: Option<&str>) -> String {
    let message = path.map_or_else(
        || localization::message(key),
        |value| localization::message(key).with_arg("path", value),
    );
    message.to_string()
}

/// One captured metric sample: its name, labels, and value.
pub(super) struct Sample {
    /// Metric name the sample was recorded under.
    name: String,
    /// Labels attached to the sample, in insertion order.
    labels: Vec<(String, String)>,
    /// Recorded counter or histogram value.
    value: DebugValue,
}

/// Build one label pair for a captured sample.
pub(super) fn label(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

/// Convert raw snapshot entries into samples, keeping the recorder's order.
pub(super) fn collect_samples(
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
pub(super) fn counter_totals(
    samples: &[Sample],
    name: &str,
) -> BTreeMap<Vec<(String, String)>, u64> {
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
pub(super) fn assert_counter_totals(
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
pub(super) fn fetch_duration_seconds(samples: &[Sample]) -> Result<Vec<f64>> {
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
