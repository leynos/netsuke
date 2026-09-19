//! Tests for the bounded `which` resolver telemetry.
//!
//! Every case drives the real [`WhichResolver`] rather than the recording
//! helpers, so the series these tests assert are the ones a manifest produces.
//! A local `DebuggingRecorder` captures the counters without touching the
//! global recorder, and the tracing cases install a temporary subscriber so
//! the span and the failure event can be inspected field by field.
//!
//! Two properties are pinned here. The `cwd_mode` label distinguishes
//! resolutions that are otherwise identical — the same tool, the same
//! directory, the same outcome — which is exactly what the counters could not
//! do before. And nothing beyond the closed vocabularies leaves the process:
//! the command, the workspace root, and the matched path are asserted absent
//! from every captured event and every captured span field.

use std::{ffi::OsString, num::NonZeroUsize, sync::Arc};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use metrics::{SharedString, Unit};
use metrics_util::CompositeKey;
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use rstest::rstest;
use tempfile::TempDir;
use tracing::level_filters::LevelFilter;

use super::{
    WhichConfig, WhichResolver,
    lookup::WorkspaceSkipList,
    options::{CwdMode, WhichOptions},
    resolve_error::ResolveError,
    telemetry::{
        CACHE_OUTCOME_BYPASS, CACHE_OUTCOME_HIT, CACHE_OUTCOME_MISS, CATEGORY_ARGS,
        CATEGORY_CANONICALIZE, CATEGORY_CANONICALIZE_NON_UTF8, CATEGORY_CWD_NON_UTF8,
        CATEGORY_CWD_RESOLVE, CATEGORY_DIRECT_NOT_FOUND, CATEGORY_IS_EXECUTABLE,
        CATEGORY_NOT_FOUND, CATEGORY_WALKDIR, CATEGORY_WORKSPACE_NON_UTF8,
        RESOLUTION_OUTCOME_FOUND, RESOLUTION_OUTCOME_NOT_FOUND, RESOLVE_ERROR_CATEGORY_VALUES,
        WHICH_CACHE_OUTCOME_VALUES, WHICH_CACHE_TOTAL, WHICH_CWD_MODE_VALUES,
        WHICH_RESOLUTION_OUTCOME_VALUES, WHICH_RESOLUTION_TOTAL, cwd_mode_label,
    },
};
use crate::test_tracing_capture::with_test_subscriber;

/// Name of the resolver span every tracing assertion targets.
const RESOLVER_SPAN: &str = "stdlib.which.resolve";

/// A temporary workspace holding the executable fixtures.
struct Workspace {
    /// Guard owning the temporary directory; dropping it removes the fixture.
    _temp: TempDir,
    /// Root of the temporary workspace.
    root: Utf8PathBuf,
    /// Name the resolver is asked for, without a platform extension.
    command: String,
}

impl Workspace {
    /// Create an empty workspace.
    fn new() -> Result<Self> {
        let temp = TempDir::new().context("create temp workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("temp path should be UTF-8: {}", path.display()))?;
        Ok(Self {
            _temp: temp,
            root,
            command: "telemetry-tool".to_owned(),
        })
    }

    /// Write the fixture executable at the workspace root.
    fn stage_tool(&self) -> Result<()> {
        test_support::write_exec(self.root.as_std_path(), &tool_filename(&self.command))
            .context("write fixture executable")?;
        Ok(())
    }

    /// Build a resolver whose PATH is exactly the supplied override.
    ///
    /// Every case passes an explicit `PATH`, so no fixture's outcome depends
    /// on the ambient environment.
    fn resolver(&self, path: Option<OsString>) -> WhichResolver {
        WhichResolver::new(WhichConfig::new(
            Some(Arc::new(self.root.clone())),
            path,
            WorkspaceSkipList::default(),
            NonZeroUsize::new(8).expect("non-zero cache capacity"),
        ))
    }
}

/// The filename the fixture executable must carry on this platform.
fn tool_filename(command: &str) -> String {
    if cfg!(windows) {
        format!("{command}.cmd")
    } else {
        command.to_owned()
    }
}

/// Join `entries` into a `PATH` value.
fn path_override(entries: &[&Utf8Path]) -> Result<OsString> {
    std::env::join_paths(entries.iter().map(|entry| entry.as_std_path()))
        .context("join PATH entries")
}

/// The default options, with the search domain set to `cwd_mode`.
fn options(cwd_mode: CwdMode) -> WhichOptions {
    WhichOptions {
        cwd_mode,
        ..WhichOptions::default()
    }
}

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

/// Each search domain maps to its own declared label.
#[rstest]
#[case::auto(CwdMode::Auto, "auto")]
#[case::always(CwdMode::Always, "always")]
#[case::never(CwdMode::Never, "never")]
#[case::workspace_recursive(CwdMode::WorkspaceRecursive, "workspace_recursive")]
fn every_search_domain_maps_to_a_declared_label(
    #[case] mode: CwdMode,
    #[case] expected: &str,
) -> Result<()> {
    ensure!(
        cwd_mode_label(mode) == expected,
        "expected {expected} but labelled {mode:?}"
    );
    ensure!(
        WHICH_CWD_MODE_VALUES.contains(&expected),
        "{expected} must be declared in the closed vocabulary"
    );
    Ok(())
}

/// The label is the telemetry spelling, and the template's is not.
///
/// A manifest writes `workspace-recursive`; the label must not, because a
/// hyphen in a metric name or label is a spelling some exporters rewrite.
/// Pinning the two spellings side by side keeps the mapping deliberate rather
/// than incidental.
#[test]
fn the_recursive_label_is_not_the_template_spelling() -> Result<()> {
    let parsed = CwdMode::parse("workspace-recursive")
        .context("the template spelling must parse to the recursive mode")?;
    ensure!(
        cwd_mode_label(parsed) == "workspace_recursive",
        "the template spelling must not become the label"
    );
    ensure!(
        !WHICH_CWD_MODE_VALUES.contains(&"workspace-recursive"),
        "the hyphenated template spelling is not a telemetry value"
    );
    Ok(())
}

/// Each closed vocabulary declares every value once.
#[test]
fn the_label_vocabularies_are_closed_and_duplicate_free() {
    for (name, values) in [
        ("cwd_mode", &WHICH_CWD_MODE_VALUES[..]),
        ("cache outcome", &WHICH_CACHE_OUTCOME_VALUES[..]),
        ("resolution outcome", &WHICH_RESOLUTION_OUTCOME_VALUES[..]),
        ("category", &RESOLVE_ERROR_CATEGORY_VALUES[..]),
    ] {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            values.len(),
            "the {name} vocabulary must declare each value once: {values:?}"
        );
    }
}

/// A `walkdir` error, produced by listing a directory that does not exist.
///
/// `walkdir::Error` has no public constructor, so the fixture is reached
/// through the walk itself.
fn walkdir_error() -> Result<walkdir::Error> {
    walkdir::WalkDir::new("/netsuke/which/telemetry/no/such/directory")
        .into_iter()
        .next()
        .context("a walk always yields at least its root")?
        .err()
        .context("a missing root must fail the walk")
}

/// Every error variant reports a declared category, and each is reachable.
///
/// Length equality is the load-bearing half: it fails both when a variant
/// reports an undeclared value and when the vocabulary declares a value no
/// variant can produce, so the label set cannot drift away from the error
/// type. The final assertion pins the spelling, because a rename that keeps
/// the set closed but changes a label would otherwise pass.
#[test]
fn every_resolve_error_variant_reports_a_declared_category() -> Result<()> {
    let path = Utf8PathBuf::from("/workspace/tool");
    let io_error = || std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let errors = [
        ResolveError::NotFound {
            command: "tool".to_owned(),
            dirs: Vec::new(),
            cwd_mode: CwdMode::Never,
        },
        ResolveError::DirectNotFound {
            command: "tool".to_owned(),
            path: path.clone(),
        },
        ResolveError::args("bad option"),
        ResolveError::Canonicalize {
            path: path.clone(),
            source: io_error(),
        },
        ResolveError::IsExecutable {
            path: path.clone(),
            source: io_error(),
        },
        ResolveError::CanonicalizeNonUtf8,
        ResolveError::WorkspaceNonUtf8 {
            command: "tool".to_owned(),
            path: path.to_string(),
        },
        ResolveError::WalkDir {
            source: walkdir_error()?,
        },
        ResolveError::CwdResolve { source: io_error() },
        ResolveError::CwdNonUtf8,
    ];

    let observed: Vec<&str> = errors.iter().map(ResolveError::category).collect();
    // `ensure!` rather than `assert!`: a `Result`-returning test reports a
    // failure by returning it, because the workspace denies
    // `clippy::panic_in_result_fn`.
    ensure!(
        observed
            .iter()
            .all(|category| RESOLVE_ERROR_CATEGORY_VALUES.contains(category)),
        "every category must be declared: {observed:?}"
    );
    ensure!(
        observed.len() == RESOLVE_ERROR_CATEGORY_VALUES.len(),
        "one category per variant, no more and no fewer: {observed:?}"
    );
    ensure!(
        RESOLVE_ERROR_CATEGORY_VALUES
            .iter()
            .all(|declared| observed.contains(declared)),
        "a declared category is unreachable: {observed:?}"
    );
    ensure!(
        observed
            == [
                CATEGORY_NOT_FOUND,
                CATEGORY_DIRECT_NOT_FOUND,
                CATEGORY_ARGS,
                CATEGORY_CANONICALIZE,
                CATEGORY_IS_EXECUTABLE,
                CATEGORY_CANONICALIZE_NON_UTF8,
                CATEGORY_WORKSPACE_NON_UTF8,
                CATEGORY_WALKDIR,
                CATEGORY_CWD_RESOLVE,
                CATEGORY_CWD_NON_UTF8,
            ],
        "the taxonomy must keep its spellings: {observed:?}"
    );
    Ok(())
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
    let resolver = workspace.resolver(Some(path_override(&[workspace.root.as_path()])?));
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
    let resolver = workspace.resolver(Some(OsString::new()));
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
    let resolver = workspace.resolver(Some(path_override(&[workspace.root.as_path()])?));
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

/// The span and the failure event carry the mode and bounded facts alone.
///
/// The command and the workspace root are the facts that must never be
/// recorded: either would name what a manifest asked for or where it looked,
/// so both are asserted absent from every captured event and span field rather
/// than merely unasserted.
#[rstest]
#[case::auto(CwdMode::Auto, "auto")]
#[case::always(CwdMode::Always, "always")]
#[case::never(CwdMode::Never, "never")]
#[case::workspace_recursive(CwdMode::WorkspaceRecursive, "workspace_recursive")]
fn the_span_and_event_carry_the_mode_and_nothing_else(
    #[case] mode: CwdMode,
    #[case] expected: &str,
) -> Result<()> {
    let workspace = Workspace::new()?;
    let resolver = workspace.resolver(Some(OsString::new()));

    let (missed, events, span) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        let missed = resolver
            .resolve(&workspace.command, &options(mode))
            .is_err();
        (
            missed,
            captured.snapshot(),
            captured.span_fields(RESOLVER_SPAN),
        )
    });

    ensure!(missed, "{expected}: the fixture should miss");
    for recorded in [
        format!("cwd_mode={expected:?}"),
        format!("cache_outcome={CACHE_OUTCOME_MISS:?}"),
        format!("result={RESOLUTION_OUTCOME_NOT_FOUND:?}"),
        format!("error_category={CATEGORY_NOT_FOUND:?}"),
    ] {
        ensure!(
            span.contains(&recorded),
            "{expected}: the span should record {recorded}: {span:?}"
        );
    }
    let failure: Vec<&String> = events
        .iter()
        .filter(|event| event.contains("which resolver finished with non-success result"))
        .collect();
    ensure!(
        failure.len() == 1,
        "{expected}: expected one failure event but captured {failure:?}"
    );
    let event = failure
        .first()
        .copied()
        .context("the length check above leaves one event")?;
    for recorded in [
        format!("cwd_mode={expected:?}"),
        format!("outcome={RESOLUTION_OUTCOME_NOT_FOUND:?}"),
        format!("error_category={CATEGORY_NOT_FOUND:?}"),
    ] {
        ensure!(
            event.contains(&recorded),
            "{expected}: the event should carry {recorded}: {event}"
        );
    }
    for captured in events.iter().chain(span.iter()) {
        ensure!(
            !captured.contains(&workspace.command) && !captured.contains(workspace.root.as_str()),
            "{expected}: no captured field may name the command or the root: {captured}"
        );
    }
    Ok(())
}
