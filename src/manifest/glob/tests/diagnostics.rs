//! Tests for the counters and tracing events glob expansion records.
//!
//! Each case exercises the manifest-template adapter or the pure query through
//! a subscriber scoped to the call. The recorder and subscriber are both
//! thread-local, so no test-wide lock is needed.

#[cfg(unix)]
use super::super::MAX_UNREACHABLE_SYMLINK_SAMPLES;
use super::super::{GlobBaseCache, PreparedGlob, expand_manifest_template_glob, glob_paths};
use super::diagnostics_support::{
    BASE_CACHE, EXPANSIONS, SKIPPED, TEMPLATE_EXPANSION_DURATION, TEMPLATE_EXPANSIONS,
    counter_value, counter_value_with_labels, has_histogram, recorded,
};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use tempfile::tempdir;
use test_support::fs as test_fs;

/// Expand and record at the manifest adapter's telemetry boundary.
fn expand_and_record(pattern: &str) -> Result<Vec<String>> {
    let base = GlobBaseCache::new(None);
    let expansion = expand_manifest_template_glob(pattern, &base)?;
    Ok(expansion.into_paths())
}

#[rstest]
fn base_cache_records_bypass_hit_miss_and_error_outcomes() -> Result<()> {
    let temporary_directory = tempdir()?;
    let base = Utf8Path::from_path(temporary_directory.path())
        .context("temporary directory should have a UTF-8 path")?
        .to_path_buf();
    let unconfigured = GlobBaseCache::new(None);
    let configured = GlobBaseCache::new(Some(base.clone()));
    let missing = GlobBaseCache::new(Some(base.join("missing")));

    let (result, events, snapshot) = recorded(|| -> Result<()> {
        PreparedGlob::new_with_base_cache("*.txt", &unconfigured)?;
        PreparedGlob::new_with_base_cache("*.txt", &configured)?;
        PreparedGlob::new_with_base_cache("*.txt", &configured)?;
        ensure!(
            PreparedGlob::new_with_base_cache("*.txt", &missing).is_err(),
            "a missing injected base must fail preparation"
        );
        Ok(())
    });
    result?;

    for outcome in ["bypass", "miss", "hit", "error"] {
        ensure!(
            counter_value(&snapshot, BASE_CACHE, ("outcome", outcome)) == Some(1),
            "expected one base-cache {outcome} outcome: {snapshot:?}"
        );
    }
    ensure!(
        events
            .iter()
            .any(|event| event.contains("operation=\"glob_base_cache\"")
                && event.contains("outcome=\"miss\"")),
        "expected the cache miss trace event: {events:?}"
    );
    ensure!(
        !events
            .iter()
            .any(|event| event.contains(&temporary_directory.path().display().to_string())),
        "base-cache trace events must not disclose the base path: {events:?}"
    );
    Ok(())
}

#[rstest]
fn a_completed_expansion_counts_its_matches() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join("a.txt"), "a")?;
    test_fs::write(temp.path().join("b.txt"), "b")?;
    let base = Utf8Path::from_path(temp.path())
        .context("temporary directory should have a UTF-8 path")?
        .to_path_buf();
    let cache = GlobBaseCache::new(Some(base));

    let (results, events, snapshot) = recorded(|| -> Result<Vec<String>> {
        let expansion = expand_manifest_template_glob("*.txt", &cache)?;
        Ok(expansion.into_paths())
    });
    ensure!(results?.len() == 2, "both files should match");

    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "matched")) == Some(1),
        "a completed expansion should count once as matched: {snapshot:?}"
    );
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[("base_mode", "injected"), ("outcome", "matched")]
        ) == Some(1),
        "the template boundary should record one injected matched result: {snapshot:?}"
    );
    ensure!(
        has_histogram(&snapshot, TEMPLATE_EXPANSION_DURATION),
        "the template boundary should record its duration: {snapshot:?}"
    );
    ensure!(
        events.iter().any(|event| {
            event.contains("operation=\"manifest_template_glob_expansion\"")
                && event.contains("base_mode=\"injected\"")
                && event.contains("outcome=\"matched\"")
                && event.contains("manifest template glob expansion completed")
        }),
        "the template boundary should emit a bounded matched trace: {events:?}"
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
    let base = Utf8Path::from_path(temp.path())
        .context("temporary directory should have a UTF-8 path")?
        .to_path_buf();
    let cache = GlobBaseCache::new(Some(base));

    let (results, events, snapshot) = recorded(|| -> Result<Vec<String>> {
        let expansion = expand_manifest_template_glob("no-such-dir/*.txt", &cache)?;
        Ok(expansion.into_paths())
    });
    ensure!(results?.is_empty(), "a missing prefix should match nothing");

    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "unopenable_prefix")) == Some(1),
        "an unopenable prefix should count once: {snapshot:?}"
    );
    ensure!(
        counter_value(&snapshot, EXPANSIONS, ("outcome", "matched")).is_none(),
        "an expansion that never ran must not count as matched: {snapshot:?}"
    );
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[("base_mode", "injected"), ("outcome", "unopenable_prefix")]
        ) == Some(1),
        "the template boundary should record one injected unopenable result: {snapshot:?}"
    );
    ensure!(
        has_histogram(&snapshot, TEMPLATE_EXPANSION_DURATION),
        "the template boundary should record its duration: {snapshot:?}"
    );
    ensure!(
        events.iter().any(|event| {
            event.contains("operation=\"manifest_template_glob_expansion\"")
                && event.contains("base_mode=\"injected\"")
                && event.contains("outcome=\"unopenable_prefix\"")
                && event.contains("manifest template glob expansion completed")
        }),
        "the template boundary should emit a bounded unopenable trace: {events:?}"
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

#[rstest]
fn a_failed_template_expansion_records_a_bounded_error_outcome() -> Result<()> {
    let cache = GlobBaseCache::new(None);

    let (result, events, snapshot) = recorded(|| expand_manifest_template_glob("[", &cache));
    ensure!(result.is_err(), "an invalid pattern must fail expansion");
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[
                ("base_mode", "process_working_directory"),
                ("outcome", "error"),
            ]
        ) == Some(1),
        "the template boundary should record one unbased error result: {snapshot:?}"
    );
    ensure!(
        has_histogram(&snapshot, TEMPLATE_EXPANSION_DURATION),
        "the failed template expansion should record its duration: {snapshot:?}"
    );
    ensure!(
        events.iter().any(|event| {
            event.contains("operation=\"manifest_template_glob_expansion\"")
                && event.contains("outcome=\"error\"")
                && event.contains("error_category=\"expansion_failure\"")
                && event.contains("manifest template glob expansion failed")
        }),
        "the failed template expansion should emit only its bounded trace: {events:?}"
    );
    ensure!(
        !events.iter().any(|event| event.contains('[')),
        "the failed template trace must not disclose the pattern: {events:?}"
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

    let (results, events, snapshot) = recorded(|| glob_paths(&pattern, None));
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
