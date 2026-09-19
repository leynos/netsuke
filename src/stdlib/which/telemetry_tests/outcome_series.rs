//! Counter-series cases for the bounded `which` resolver telemetry.
//!
//! Each case resolves a fixture through the real resolver under a local
//! `DebuggingRecorder` and then reads the counter series back as bounded
//! labels. The claim under test is always the same one: the `cwd_mode` label
//! separates two resolutions that are otherwise indistinguishable, and the
//! sample set stays inside the declared vocabularies.

use std::ffi::OsString;

use anyhow::{Result, ensure};
use metrics::{SharedString, Unit};
use metrics_util::CompositeKey;
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use rstest::rstest;

use super::super::{
    options::{CwdMode, WhichOptions},
    resolve_error::ResolveError,
    telemetry::{
        CACHE_OUTCOME_BYPASS, CACHE_OUTCOME_HIT, CACHE_OUTCOME_MISS, CATEGORY_NOT_FOUND,
        RESOLUTION_OUTCOME_FOUND, RESOLUTION_OUTCOME_NOT_FOUND, WHICH_CACHE_TOTAL,
        WHICH_RESOLUTION_TOTAL,
    },
};
use super::{Workspace, options, path_override};

/// One counter sample, flattened to the bounded labels that produced it.
#[derive(Debug, PartialEq, Eq)]
struct Sample {
    /// The `cwd_mode` label.
    cwd_mode: String,
    /// The `outcome` label.
    outcome: String,
    /// The `category` label, present only on a resolution failure.
    category: Option<String>,
    /// The recorded count.
    count: u64,
}

impl Sample {
    /// Describe one series with a count of one.
    fn once(cwd_mode: &str, outcome: &str, category: Option<&str>) -> Self {
        Self {
            cwd_mode: cwd_mode.to_owned(),
            outcome: outcome.to_owned(),
            category: category.map(str::to_owned),
            count: 1,
        }
    }

    /// The series a failed resolution records.
    fn failure(cwd_mode: &str) -> Self {
        Self::once(
            cwd_mode,
            RESOLUTION_OUTCOME_NOT_FOUND,
            Some(CATEGORY_NOT_FOUND),
        )
    }
}

/// One snapshot entry, as the debugging recorder renders it.
type Entry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

/// Every counter sample one read of the recorder reported.
///
/// The recorder answers a snapshot with each counter's delta since the last
/// snapshot, so a series read twice reports its count once and then zero.
/// Holding the read in a value makes that explicit: a test takes `Samples`
/// once and queries it, rather than reading the recorder again and silently
/// comparing against a zeroed series.
struct Samples {
    /// The counter samples, sorted by metric name and then by their labels.
    counters: Vec<(&'static str, Sample)>,
}

impl Samples {
    /// Read every bounded counter series from `snapshotter`.
    ///
    /// Call this once per test. The recorder keys its snapshot by metric name
    /// and label set, so one read yields both counters; the order within a
    /// counter is the hasher's, which the sort below replaces with a stable
    /// label order rather than pretending to recover the call order.
    fn take(snapshotter: &Snapshotter) -> Self {
        let mut counters: Vec<(&'static str, Sample)> = snapshotter
            .snapshot()
            .into_vec()
            .into_iter()
            .filter_map(|entry: Entry| {
                let (key, _unit, _description, value) = entry;
                if key.kind() != MetricKind::Counter {
                    return None;
                }
                let name = match key.key().name() {
                    WHICH_CACHE_TOTAL => WHICH_CACHE_TOTAL,
                    WHICH_RESOLUTION_TOTAL => WHICH_RESOLUTION_TOTAL,
                    _ => return None,
                };
                let label = |field: &str| {
                    key.key()
                        .labels()
                        .find(|label| label.key() == field)
                        .map(|label| label.value().to_owned())
                };
                let DebugValue::Counter(count) = value else {
                    return None;
                };
                Some((
                    name,
                    Sample {
                        cwd_mode: label("cwd_mode")?,
                        outcome: label("outcome")?,
                        category: label("category"),
                        count,
                    },
                ))
            })
            .collect();
        counters.sort_unstable_by(|left, right| {
            left.0.cmp(right.0).then_with(|| {
                (&left.1.cwd_mode, &left.1.outcome, &left.1.category).cmp(&(
                    &right.1.cwd_mode,
                    &right.1.outcome,
                    &right.1.category,
                ))
            })
        });
        Self { counters }
    }

    /// The samples of one counter, ordered by their labels.
    fn of(&self, metric: &'static str) -> Vec<Sample> {
        self.counters
            .iter()
            .filter(|(name, _sample)| *name == metric)
            .map(|(_name, sample)| Sample {
                cwd_mode: sample.cwd_mode.clone(),
                outcome: sample.outcome.clone(),
                category: sample.category.clone(),
                count: sample.count,
            })
            .collect()
    }
}

/// A hit is attributed to the domain that produced it.
///
/// All four cases search one directory and find the same tool, so the only
/// thing that can separate their series is the `cwd_mode` label — the
/// distinction the counters previously lacked.
#[rstest]
#[case::auto(CwdMode::Auto, "auto")]
#[case::always(CwdMode::Always, "always")]
#[case::never(CwdMode::Never, "never")]
#[case::workspace_recursive(CwdMode::WorkspaceRecursive, "workspace_recursive")]
fn a_hit_is_labelled_with_its_search_domain(
    #[case] mode: CwdMode,
    #[case] expected: &str,
) -> Result<()> {
    let workspace = Workspace::new()?;
    workspace.stage_tool()?;
    let resolver = workspace.resolver(Some(path_override(&[workspace.root.as_path()])?))?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let matches = metrics::with_local_recorder(&recorder, || {
        resolver.resolve(&workspace.command, &options(mode))
    })?;

    let samples = Samples::take(&snapshotter);
    ensure!(!matches.is_empty(), "{expected}: the tool should resolve");
    ensure!(
        samples.of(WHICH_CACHE_TOTAL) == [Sample::once(expected, CACHE_OUTCOME_MISS, None)],
        "{expected}: the cold lookup should be one miss"
    );
    ensure!(
        samples.of(WHICH_RESOLUTION_TOTAL)
            == [Sample::once(expected, RESOLUTION_OUTCOME_FOUND, None)],
        "{expected}: the resolution should be one bounded found series"
    );
    Ok(())
}

/// A miss is attributed to the domain that produced it.
#[rstest]
#[case::auto(CwdMode::Auto, "auto")]
#[case::always(CwdMode::Always, "always")]
#[case::never(CwdMode::Never, "never")]
#[case::workspace_recursive(CwdMode::WorkspaceRecursive, "workspace_recursive")]
fn a_miss_is_labelled_with_its_search_domain(
    #[case] mode: CwdMode,
    #[case] expected: &str,
) -> Result<()> {
    let workspace = Workspace::new()?;
    let resolver = workspace.resolver(Some(OsString::new()))?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let result = metrics::with_local_recorder(&recorder, || {
        resolver.resolve(&workspace.command, &options(mode))
    });

    let samples = Samples::take(&snapshotter);
    ensure!(
        matches!(result, Err(ResolveError::NotFound { .. })),
        "{expected}: an empty PATH holds no fixture, so the lookup should miss"
    );
    ensure!(
        samples.of(WHICH_CACHE_TOTAL) == [Sample::once(expected, CACHE_OUTCOME_MISS, None)],
        "{expected}: the cold lookup should be one miss"
    );
    ensure!(
        samples.of(WHICH_RESOLUTION_TOTAL) == [Sample::failure(expected)],
        "{expected}: the miss should carry its bounded category"
    );
    Ok(())
}

/// Every cache outcome carries the mode, and repeats tally.
///
/// Three passes over the same tool in one domain: a cold miss that populates
/// the cache, a hit, and a fresh resolution that bypasses it. The resolution
/// counter sees one label set three times, so its sample must be a tally
/// rather than three series; the sorted order below is `bypass`, `hit`, `miss`
/// because the samples are ordered by their labels.
#[test]
fn cache_outcomes_carry_the_search_domain() -> Result<()> {
    let workspace = Workspace::new()?;
    workspace.stage_tool()?;
    let resolver = workspace.resolver(Some(path_override(&[workspace.root.as_path()])?))?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let mode = CwdMode::WorkspaceRecursive;

    let outcomes = metrics::with_local_recorder(&recorder, || {
        let cold = resolver.resolve(&workspace.command, &options(mode)).is_ok();
        let warm = resolver.resolve(&workspace.command, &options(mode)).is_ok();
        let bypassed = resolver
            .resolve(
                &workspace.command,
                &WhichOptions {
                    fresh: true,
                    ..options(mode)
                },
            )
            .is_ok();
        [cold, warm, bypassed]
    });

    let samples = Samples::take(&snapshotter);
    ensure!(
        outcomes == [true, true, true],
        "the fixture should resolve on every pass: {outcomes:?}"
    );
    ensure!(
        samples.of(WHICH_CACHE_TOTAL)
            == [
                Sample::once("workspace_recursive", CACHE_OUTCOME_BYPASS, None),
                Sample::once("workspace_recursive", CACHE_OUTCOME_HIT, None),
                Sample::once("workspace_recursive", CACHE_OUTCOME_MISS, None),
            ],
        "each cache outcome should be attributed to the mode"
    );
    let found = samples.of(WHICH_RESOLUTION_TOTAL);
    ensure!(
        found
            == [Sample {
                cwd_mode: "workspace_recursive".to_owned(),
                outcome: RESOLUTION_OUTCOME_FOUND.to_owned(),
                category: None,
                count: 3,
            }],
        "the three passes are one series, so the count must be a tally: {found:?}"
    );
    Ok(())
}
