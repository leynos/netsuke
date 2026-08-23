//! Tests for the counters and tracing events glob expansion records.
//!
//! Each case records data returned by the pure expansion query through a
//! subscriber scoped to the call. The recorder and subscriber are both
//! thread-local, so no test-wide lock is needed.

#[cfg(unix)]
use super::super::MAX_UNREACHABLE_SYMLINK_SAMPLES;
use super::super::{expand_glob, glob_paths, record_expansion};
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

/// Expand and record at the manifest adapter's telemetry boundary.
fn expand_and_record(pattern: &str) -> Result<Vec<String>> {
    let expansion = expand_glob(pattern)?;
    record_expansion(&expansion);
    Ok(expansion.into_paths())
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

    let (results, events, snapshot) = recorded(|| expand_and_record(&pattern));
    ensure!(results?.len() == 2, "both files should match");

    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "matched")) == Some(1),
        "a completed expansion should count once as matched: {snapshot:?}"
    );
    let expansion_event = events
        .iter()
        .find(|event| event.contains("glob expansion complete"))
        .context("expected a completed-expansion event")?;
    ensure!(
        expansion_event.contains("matches=2") && expansion_event.contains("pattern=\"<redacted>\""),
        "expected bounded fields in the trace event: {expansion_event}"
    );
    ensure!(
        !expansion_event.contains(&temp.path().display().to_string()),
        "the event must not disclose the absolute path: {expansion_event}"
    );
    Ok(())
}

#[rstest]
fn an_unopenable_prefix_counts_and_names_the_prefix() -> Result<()> {
    let temp = tempdir()?;
    let pattern = format!("{}/no-such-dir/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| expand_and_record(&pattern));
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
        prefix_event.contains("pattern=\"<redacted>\"")
            && prefix_event.contains("prefix=\"<redacted>\""),
        "expected bounded fields in the trace event: {prefix_event}"
    );
    ensure!(
        !prefix_event.contains(&temp.path().display().to_string()),
        "the event must not disclose the absolute path: {prefix_event}"
    );
    Ok(())
}

/// A skipped match is counted by reason and traced without its path.
#[cfg(unix)]
#[rstest]
fn a_skipped_symlink_counts_and_redacts_its_relative_path() -> Result<()> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    let vendor = temp.path().join("vendor");
    test_fs::create_dir(&src)?;
    test_fs::create_dir(&vendor)?;
    test_fs::write(vendor.join("escaped.txt"), "escaped")?;
    test_fs::symlink("../vendor/escaped.txt", src.join("escaped.txt"))?;
    let pattern = format!("{}/src/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| expand_and_record(&pattern));
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
        skip_event.contains("relative=\"<redacted>\""),
        "the event should carry a redacted relative path: {skip_event}"
    );
    ensure!(
        !skip_event.contains(&temp.path().display().to_string()),
        "the event must not disclose the absolute path: {skip_event}"
    );
    ensure!(
        !skip_event.contains("escaped.txt"),
        "the event must not disclose the relative path: {skip_event}"
    );
    Ok(())
}

/// All skipped links contribute to metrics, but traces retain only four entries.
#[cfg(unix)]
#[rstest]
fn skipped_symlink_diagnostics_retain_a_bounded_sample() -> Result<()> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    let vendor = temp.path().join("vendor");
    test_fs::create_dir(&src)?;
    test_fs::create_dir(&vendor)?;
    let skipped_count = MAX_UNREACHABLE_SYMLINK_SAMPLES + 2;
    for index in 0..skipped_count {
        let name = format!("escaped-{index:02}.txt");
        test_fs::write(vendor.join(&name), "escaped")?;
        test_fs::symlink(format!("../vendor/{name}"), src.join(name))?;
    }
    let pattern = format!("{}/src/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| expand_and_record(&pattern));
    ensure!(results?.is_empty(), "every match should be skipped");
    ensure!(
        counter_value(&snapshot, SKIPPED, ("reason", "unreachable_symlink"))
            == Some(skipped_count as u64),
        "the aggregate counter should include every skipped link: {snapshot:?}"
    );
    let sampled_events: Vec<_> = events
        .iter()
        .filter(|event| event.contains("cannot resolve"))
        .collect();
    ensure!(
        sampled_events.len() == MAX_UNREACHABLE_SYMLINK_SAMPLES,
        "expected exactly the bounded trace sample: {sampled_events:?}"
    );
    for sampled_event in &sampled_events {
        ensure!(
            sampled_event.contains("relative=\"<redacted>\""),
            "sampled events must redact their paths: {sampled_events:?}"
        );
    }
    ensure!(
        !sampled_events
            .iter()
            .any(|event| event.contains("escaped-")),
        "sampled events must not disclose retained paths: {sampled_events:?}"
    );
    Ok(())
}

#[rstest]
fn a_relative_unopenable_prefix_redacts_caller_controlled_fields() -> Result<()> {
    let pattern = "glob-diagnostics-no-such-prefix/*.txt";

    let (results, events, _snapshot) = recorded(|| expand_and_record(pattern));
    ensure!(results?.is_empty(), "a missing prefix should match nothing");

    let prefix_event = events
        .iter()
        .find(|event| event.contains("glob literal prefix names no directory"))
        .context("expected an unopenable-prefix event")?;
    ensure!(
        prefix_event.contains("pattern=\"<redacted>\"")
            && prefix_event.contains("prefix=\"<redacted>\""),
        "expected redacted fields in the trace event: {prefix_event}"
    );
    ensure!(
        !prefix_event.contains("glob-diagnostics-no-such-prefix"),
        "the event must not disclose the relative pattern: {prefix_event}"
    );
    Ok(())
}

#[rstest]
fn a_directory_match_counts_as_not_a_file() -> Result<()> {
    let temp = tempdir()?;
    test_fs::create_dir(temp.path().join("sub"))?;
    test_fs::write(temp.path().join("a.txt"), "a")?;
    let pattern = format!("{}/*", temp.path().display());

    let (results, _events, snapshot) = recorded(|| expand_and_record(&pattern));
    ensure!(results?.len() == 1, "only the file should survive");

    ensure!(
        counter_value(&snapshot, SKIPPED, ("reason", "not_a_file")) == Some(1),
        "the directory should count once as not a file: {snapshot:?}"
    );
    Ok(())
}

/// Direct callers can reuse the glob query without receiving global telemetry.
#[rstest]
fn glob_paths_is_a_pure_query() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join("a.txt"), "a")?;
    let pattern = format!("{}/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| glob_paths(&pattern));
    ensure!(results?.len() == 1, "the file should match");
    ensure!(
        events.is_empty(),
        "the query must not emit trace events: {events:?}"
    );
    ensure!(
        snapshot.is_empty(),
        "the query must not record metrics: {snapshot:?}"
    );
    Ok(())
}
