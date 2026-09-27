//! Failure-category cases for the bounded `which` resolver telemetry.
//!
//! The two sibling modules drive a `PATH` search miss, which is one point of
//! the failure taxonomy: the outcome `not_found` under the category
//! `not_found`. Two further points are reachable through the resolver and are
//! pinned nowhere else. Each is driven here end to end — the counter series,
//! the resolver span, and the failure event.
//!
//! Every expected label is written here as its externally reported spelling,
//! not imported from the module that emits it. That is deliberate and it is the
//! whole value of these cases: the resolver builds both the constant and the
//! recorded label from the same `&str`, so an expectation taken from that
//! constant is the implementation compared against itself. Renaming `not_found`
//! to something else would move both sides at once and leave the case green
//! while every dashboard and alert that reads the old spelling broke. Pinning
//! the literal is what makes a rename fail here instead of in production.
//!
//! The two are the direct-path miss, which is a miss that was never a search,
//! and the executable-probe failure, which is not a miss at all: the resolver
//! knows that inspecting the path failed rather than that nothing was found.
//! The resolution counter separates them by an `outcome` label — `not_found`
//! against `error` — and by a category, and an operator needs both halves:
//! searching more widely is the right answer to one and a wasted search to the
//! other.
//!
//! The sample fixtures are the ones [`outcome_series`] reads its cases with,
//! and the failure message is the one [`tracing_capture`] asserts on. Sharing
//! them is deliberate: a second reader of the recorder would be a second
//! definition of what a bounded series is, and a second copy of the message
//! would drift from the text a reader sees.

use std::ffi::OsString;

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use metrics_util::debugging::DebuggingRecorder;
use tracing::level_filters::LevelFilter;

use super::super::{
    WhichResolver,
    options::CwdMode,
    resolve_error::ResolveError,
    telemetry::{WHICH_CACHE_TOTAL, WHICH_RESOLUTION_TOTAL},
};
use super::outcome_series::{Sample, Samples};
use super::tracing_capture::FAILURE_MESSAGE;
use super::{RESOLVER_SPAN, Workspace, options};
use crate::test_tracing_capture::with_test_subscriber;
#[cfg(unix)]
use camino::Utf8Path;

/// The path-like command the direct-path miss case is driven with.
///
/// Deliberately not the name the workspace fixture stages: the case is about a
/// path that is not there, so it must not be able to become a hit because
/// something else staged a tool of that name. The separator is the point — it
/// is what sends the lookup down the direct-path branch rather than into a
/// `PATH` search.
const ABSENT_DIRECT_PATH: &str = "./absent-tool";

/// The externally reported `cache_outcome` for a cold miss.
///
/// Written out rather than imported, for the reason the module document gives:
/// `cache.rs` records this value from the same constant the resolver exports, so
/// an expectation taken from that constant would move with it. The literal is
/// what makes a rename of the recorded spelling fail here.
const CACHE_MISS: &str = "miss";

/// The executable the probe-failure case is driven with.
///
/// Named rather than inlined because the value is a claim about the fixture on
/// disk: it is written inside the restricted directory by this name, and the
/// resolution asks for it by the same one.
#[cfg(unix)]
const UNINSPECTABLE_TOOL: &str = "restricted-tool";

/// The bounded facts one failing resolution must report on all three subjects.
///
/// Held together because they are one claim read three ways: the counter, the
/// span, and the event describe a single failure, and a category recorded on
/// one but not the others is exactly the drift a case reading one subject
/// cannot show.
struct ExpectedFailure {
    /// The bounded `cwd_mode` the resolution was requested under.
    cwd_mode: &'static str,
    /// The bounded `outcome` the resolution counter must carry.
    outcome: &'static str,
    /// The bounded `category` the counter, the span, and the event must carry.
    category: &'static str,
}

/// What one resolution left behind on each subject it reports through.
struct Observed {
    /// The resolver's own verdict, still typed so a case can name the variant.
    result: Result<Vec<Utf8PathBuf>, ResolveError>,
    /// The counter samples the resolution recorded, read once.
    samples: Samples,
    /// Every event the resolution emitted, in the order it emitted them.
    events: Vec<String>,
    /// The fields the resolver's span recorded.
    span: Vec<String>,
}

/// Drive one resolution and read back its counters, span, and events.
///
/// The recorder and the subscriber are installed around the same call, so the
/// three subjects describe one resolution rather than three. The samples are
/// read once, after the call and outside the recorder closure: the recorder
/// reports each series' delta since the last read, so a second read would
/// compare against a zeroed series.
fn read_resolution(resolver: &WhichResolver, command: &str, cwd_mode: CwdMode) -> Observed {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (result, events, span) = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::TRACE, |captured| {
            let result = resolver.resolve(command, &options(cwd_mode));
            (
                result,
                captured.snapshot(),
                captured.span_fields(RESOLVER_SPAN),
            )
        })
    });
    Observed {
        result,
        samples: Samples::take(&snapshotter),
        events,
        span,
    }
}

/// The span fields a failed resolution records, and no others.
///
/// Four, where a successful resolution records three: a failure adds both a
/// nonzero `result` and an `error_category`. Pinning the whole set is what
/// makes a field the resolver was never meant to record a failure, where
/// looking for the expected fields inside whatever was captured would never
/// find one.
fn expected_span_fields(expected: &ExpectedFailure) -> Vec<String> {
    let mut fields = vec![
        format!("cache_outcome={CACHE_MISS:?}"),
        format!("cwd_mode={:?}", expected.cwd_mode),
        format!("error_category={:?}", expected.category),
        format!("result={:?}", expected.outcome),
    ];
    fields.sort_unstable();
    fields
}

/// Assert the counter series a failed resolution records.
///
/// Both counters are read whole. The cache counter is pinned to its one series,
/// and the resolution counter to a single sample holding the *entire* label set
/// the recorder reported: an extra label the resolver was never meant to emit
/// changes that sample and fails the case, where reading only the labels named
/// here would project it onto a bounded shape and compare it equal.
fn assert_failure_counters(expected: &ExpectedFailure, observed: &Observed) -> Result<()> {
    let mode = expected.cwd_mode;
    ensure!(
        observed.samples.of(WHICH_CACHE_TOTAL) == [Sample::once(mode, CACHE_MISS, None)],
        "{mode}: the failed lookup should be one cold miss: {:?}",
        observed.samples.of(WHICH_CACHE_TOTAL)
    );
    let resolutions = observed.samples.of(WHICH_RESOLUTION_TOTAL);
    ensure!(
        resolutions
            == [Sample::once(
                mode,
                expected.outcome,
                Some(expected.category)
            )],
        "{mode}: the failure should be one series carrying its outcome and its \
         own category: {resolutions:?}"
    );
    Ok(())
}

/// Assert the span a failed resolution records, as an exact field set.
fn assert_failure_span(expected: &ExpectedFailure, observed: &Observed) -> Result<()> {
    let mut recorded = observed.span.clone();
    recorded.sort_unstable();
    ensure!(
        recorded == expected_span_fields(expected),
        "{}: the span must record the bounded fields and nothing else: {:?}",
        expected.cwd_mode,
        observed.span
    );
    Ok(())
}

/// Assert the failure event a resolution emitted carries the bounded facts.
///
/// Exactly one event carries the failure message, which the sibling module
/// asserts too: two would be two accounts of one failure, and only one of them
/// would be the one the next assertion holds to an exact field set. The message
/// is removed first — it is the one part of a rendered event that is not a
/// `name=value` pair — and what remains is compared as a set, so a field
/// appended to the event has nowhere to hide, while a reordering of the fields
/// the resolver *does* record is not a failure.
fn assert_failure_event(expected: &ExpectedFailure, observed: &Observed) -> Result<()> {
    let mode = expected.cwd_mode;
    let failures: Vec<&String> = observed
        .events
        .iter()
        .filter(|event| event.contains(FAILURE_MESSAGE))
        .collect();
    ensure!(
        failures.len() == 1,
        "{mode}: expected one failure event but captured {failures:?}"
    );
    let event = failures
        .first()
        .copied()
        .context("the length check above leaves one failure event")?;

    let remainder = event.replacen(&format!("message={FAILURE_MESSAGE}"), "", 1);
    let mut recorded: Vec<String> = remainder.split_whitespace().map(str::to_owned).collect();
    recorded.sort_unstable();
    let mut wanted = vec![
        format!("cwd_mode={mode:?}"),
        format!("outcome={:?}", expected.outcome),
        format!("error_category={:?}", expected.category),
    ];
    wanted.sort_unstable();
    ensure!(
        recorded == wanted,
        "{mode}: the event must carry the bounded facts and nothing else, but \
         recorded {recorded:?}: {event}"
    );
    Ok(())
}

/// Assert no captured field on any subject names `sensitive`.
///
/// The exact comparisons above already exclude these values from the two
/// subjects whose field *sets* they pin. Asserting them here as well states
/// which values are sensitive and covers anything captured outside those
/// subjects, so a field added on some other path is still held to the same
/// redaction claim. An empty value is refused rather than asserted absent: a
/// containment check against nothing is satisfied by every field and would read
/// as a pass.
fn assert_unnamed(observed: &Observed, sensitive: &str) -> Result<()> {
    for captured in observed.events.iter().chain(observed.span.iter()) {
        ensure!(
            !sensitive.is_empty() && !captured.contains(sensitive),
            "no captured field may name {sensitive}: {captured}"
        );
    }
    Ok(())
}

/// Assert a failure reports the same bounded facts on every subject.
fn assert_failure(expected: &ExpectedFailure, observed: &Observed) -> Result<()> {
    assert_failure_counters(expected, observed)?;
    assert_failure_span(expected, observed)?;
    assert_failure_event(expected, observed)
}

/// A direct-path miss carries the category that separates it from a search.
///
/// Both misses arrive as the outcome `not_found` and both make the command
/// unavailable, so the category is the only fact that tells "the `PATH` held
/// nothing" from "the path you named is not an executable". Those are different
/// answers for an operator: one justifies searching more widely, and the other
/// never will, because the second lookup would be handed exactly the same path.
#[test]
fn a_direct_path_miss_carries_the_category_that_separates_it_from_a_search() -> Result<()> {
    let workspace = Workspace::new()?;
    // An empty `PATH`, so nothing but the direct-path branch can produce the
    // failure: the category under test is not the search miss's.
    let resolver = workspace.resolver(Some(OsString::new()))?;

    let recorded = read_resolution(&resolver, ABSENT_DIRECT_PATH, CwdMode::Never);

    ensure!(
        matches!(recorded.result, Err(ResolveError::DirectNotFound { .. })),
        "a path-like command naming nothing should miss directly: {:?}",
        recorded.result
    );
    assert_failure(
        &ExpectedFailure {
            cwd_mode: "never",
            outcome: "not_found",
            category: "direct_not_found",
        },
        &recorded,
    )?;
    // `DirectNotFound` carries the candidate it resolved to — a real path on
    // this host — so the command's own text and that resolved path are the two
    // forms that could reach a field.
    assert_unnamed(&recorded, ABSENT_DIRECT_PATH)?;
    assert_unnamed(&recorded, workspace.root.join(ABSENT_DIRECT_PATH).as_str())
}

/// An executable probe that fails is an error, not a miss.
///
/// The fixture is a file that exists and would be accepted; only the mode of
/// its directory withholds it. `fs::metadata` therefore fails rather than
/// reporting the path absent, which is the distinction the case turns on: a
/// path that is not there means "not an executable", and a path that cannot be
/// inspected means the resolver does not know. Recording the second as
/// `not_found` would send an operator to search harder for something that is
/// sitting exactly where they said it was.
///
/// POSIX-only, and gated per test rather than per module: the fixture is a
/// permission bit, which no other platform has. A module-level gate would drop
/// the case above from the suite on those platforms instead of from this one.
#[cfg(unix)]
#[test]
fn an_uninspectable_path_is_an_error_rather_than_a_miss() -> Result<()> {
    let workspace = Workspace::new()?;
    let directory = workspace.root.join("restricted");
    let tool = directory.join(UNINSPECTABLE_TOOL);
    stage_uninspectable_tool(&directory, &tool)?;
    let resolver = workspace.resolver(Some(OsString::new()))?;

    let recorded = read_resolution(&resolver, tool.as_str(), CwdMode::Never);

    // Restored before anything is asserted. A temporary tree whose child denies
    // search cannot be removed, and the guard that owns the tree drops on
    // return — so a failing assertion must not be able to leave it behind.
    restore_directory(&directory)?;

    ensure!(
        matches!(recorded.result, Err(ResolveError::IsExecutable { .. })),
        "an uninspectable path should fail the probe: {:?}",
        recorded.result
    );
    assert_failure(
        &ExpectedFailure {
            cwd_mode: "never",
            outcome: "error",
            category: "is_executable",
        },
        &recorded,
    )?;
    // `IsExecutable` names the path it could not inspect, and that path names
    // the directory the resolver looked in. Neither may reach a field.
    assert_unnamed(&recorded, tool.as_str())?;
    assert_unnamed(&recorded, directory.as_str())?;

    // The control. The same resolver, asked for the same path once the mode is
    // back, resolves it, so the failure above was the permission and nothing
    // else: the case cannot be passing because the fixture was never there.
    let control = resolver.resolve(tool.as_str(), &options(CwdMode::Never))?;
    ensure!(
        control == [tool],
        "the restored fixture should resolve to itself: {control:?}"
    );
    Ok(())
}

/// Write an executable into `directory` and withhold the search bit on it.
///
/// The order matters: the tool is written and marked executable while the
/// directory is still open, then the directory is restricted, so the failure
/// the caller observes is the probe's and not the fixture's.
#[cfg(unix)]
fn stage_uninspectable_tool(directory: &Utf8Path, tool: &Utf8Path) -> Result<()> {
    test_support::fs::create_dir_all(directory.as_std_path())
        .with_context(|| format!("create {directory}"))?;
    test_support::write_exec(directory.as_std_path(), UNINSPECTABLE_TOOL)
        .with_context(|| format!("write fixture executable {tool}"))?;
    restrict_directory(directory)
}

/// Withhold the search bit on `directory`, so its contents cannot be probed.
///
/// `0o600` grants read and write but not search, which is what turns a probe of
/// a child into an [`std::io::ErrorKind::PermissionDenied`] rather than a
/// verdict about the child.
#[cfg(unix)]
fn restrict_directory(directory: &Utf8Path) -> Result<()> {
    test_support::fs::set_mode(directory.as_std_path(), 0o600)
        .with_context(|| format!("restrict {directory}"))
}

/// Restore the search bit on `directory` so its tree can be removed.
#[cfg(unix)]
fn restore_directory(directory: &Utf8Path) -> Result<()> {
    test_support::fs::set_mode(directory.as_std_path(), 0o700)
        .with_context(|| format!("restore {directory}"))
}
