//! Tests for the counters and tracing events glob expansion records.
//!
//! Each case drives `glob_paths` through a recorder and a subscriber scoped to
//! the call, so it fails if the corresponding `record_*` call is removed. The
//! recorder and subscriber are both thread-local, so no test-wide lock is
//! needed.

use super::super::glob_paths;
use anyhow::{Context, Result, ensure};
use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;
use tempfile::tempdir;
use test_support::fs as test_fs;
use tracing::level_filters::LevelFilter;

type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// Run `expand` with a local metrics recorder and a capturing subscriber.
fn recorded<T>(expand: impl FnOnce() -> T) -> (T, Vec<String>, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (value, events) = metrics::with_local_recorder(&recorder, || {
        crate::test_tracing_capture::with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let value = expand();
            (value, captured.snapshot())
        })
    });
    (value, events, snapshotter.snapshot().into_vec())
}

/// Value of the counter `name` carrying the label `label = value`.
fn counter_value(snapshot: &Snapshot, name: &str, label: (&str, &str)) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, debug_value)| {
        if key.kind() != MetricKind::Counter || key.key().name() != name {
            return None;
        }
        let carries_label = key
            .key()
            .labels()
            .any(|found| found.key() == label.0 && found.value() == label.1);
        match debug_value {
            DebugValue::Counter(count) if carries_label => Some(*count),
            _ => None,
        }
    })
}

const EXPANSIONS: &str = "netsuke_manifest_glob_expansions_total";
const SKIPPED: &str = "netsuke_manifest_glob_entries_skipped_total";

#[rstest]
fn a_completed_expansion_counts_its_matches() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join("a.txt"), "a")?;
    test_fs::write(temp.path().join("b.txt"), "b")?;
    let pattern = format!("{}/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| glob_paths(&pattern));
    ensure!(results?.len() == 2, "both files should match");

    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "matched")) == Some(1),
        "a completed expansion should count once as matched: {snapshot:?}"
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("glob expansion complete") && event.contains("matches=2")),
        "expected the match count in a trace event: {events:?}"
    );
    Ok(())
}

#[rstest]
fn an_unopenable_prefix_counts_and_names_the_prefix() -> Result<()> {
    let temp = tempdir()?;
    let pattern = format!("{}/no-such-dir/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| glob_paths(&pattern));
    ensure!(results?.is_empty(), "a missing prefix should match nothing");

    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "unopenable_prefix")) == Some(1),
        "an unopenable prefix should count once: {snapshot:?}"
    );
    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "matched")).is_none(),
        "an expansion that never ran must not count as matched: {snapshot:?}"
    );
    let prefix_event = events
        .iter()
        .find(|event| event.contains("glob literal prefix names no directory"))
        .context("expected an unopenable-prefix event")?;
    ensure!(
        prefix_event.contains("no-such-dir"),
        "the event should name the prefix: {prefix_event}"
    );
    Ok(())
}

/// A skipped match is counted by reason and traced by its path relative to the
/// prefix — never by its absolute path.
#[cfg(unix)]
#[rstest]
fn a_skipped_symlink_counts_and_traces_only_the_relative_path() -> Result<()> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    let vendor = temp.path().join("vendor");
    test_fs::create_dir(&src)?;
    test_fs::create_dir(&vendor)?;
    test_fs::write(vendor.join("escaped.txt"), "escaped")?;
    test_fs::symlink("../vendor/escaped.txt", src.join("escaped.txt"))?;
    let pattern = format!("{}/src/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| glob_paths(&pattern));
    ensure!(results?.is_empty(), "the only match should be skipped");

    ensure!(
        counter_value(&snapshot, SKIPPED, ("reason", "unreachable_symlink")) == Some(1),
        "the skipped link should count once: {snapshot:?}"
    );
    let skip_event = events
        .iter()
        .find(|event| event.contains("cannot resolve"))
        .context("expected a skipped-match event")?;
    ensure!(
        skip_event.contains("relative=escaped.txt"),
        "the event should carry the prefix-relative path: {skip_event}"
    );
    ensure!(
        !skip_event.contains(&temp.path().display().to_string()),
        "the event must not disclose the absolute path: {skip_event}"
    );
    Ok(())
}

#[rstest]
fn a_directory_match_counts_as_not_a_file() -> Result<()> {
    let temp = tempdir()?;
    test_fs::create_dir(temp.path().join("sub"))?;
    test_fs::write(temp.path().join("a.txt"), "a")?;
    let pattern = format!("{}/*", temp.path().display());

    let (results, _events, snapshot) = recorded(|| glob_paths(&pattern));
    ensure!(results?.len() == 1, "only the file should survive");

    ensure!(
        counter_value(&snapshot, SKIPPED, ("reason", "not_a_file")) == Some(1),
        "the directory should count once as not a file: {snapshot:?}"
    );
    Ok(())
}
