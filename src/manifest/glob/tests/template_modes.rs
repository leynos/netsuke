//! Tests for manifest-template glob telemetry classifications.
//!
//! These cases exercise the template boundary's bounded base-mode and failure
//! outcomes without widening the telemetry-free direct query tests.

use super::super::super::{GlobBaseCache, GlobExpansion, expand_manifest_template_glob};
use super::super::diagnostics_support::{
    BASE_CACHE, TEMPLATE_EXPANSION_DURATION, TEMPLATE_EXPANSIONS, counter_value,
    counter_value_with_labels, has_histogram, recorded,
};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use tempfile::tempdir;
use test_support::fs as test_fs;

/// Verify that absolute patterns do not prepare the configured injected base.
#[rstest]
fn an_absolute_pattern_bypasses_an_injected_base_cache() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join("a.txt"), "a")?;
    let base = Utf8Path::from_path(temp.path())
        .context("temporary directory should have a UTF-8 path")?
        .to_path_buf();
    let cache = GlobBaseCache::new(Some(base));
    let pattern = format!("{}/*.txt", temp.path().display());

    let (results, events, snapshot) = recorded(|| -> Result<Vec<String>> {
        let expansion = expand_manifest_template_glob(&pattern, &cache)?;
        Ok(GlobExpansion::into_paths(expansion))
    });
    ensure!(
        results?.len() == 1,
        "the absolute pattern should match its file"
    );
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[("base_mode", "absolute_pattern"), ("outcome", "matched")]
        ) == Some(1),
        "the template boundary should classify the absolute pattern: {snapshot:?}"
    );
    ensure!(
        counter_value(&snapshot, BASE_CACHE, ("outcome", "miss")).is_none()
            && !events
                .iter()
                .any(|event| event.contains("manifest glob base canonicalized and cached")),
        "an absolute pattern must not canonicalize an injected base: {events:?} {snapshot:?}"
    );
    Ok(())
}

/// Verify that unbased relative patterns report their process-rooted mode.
#[rstest]
fn a_relative_pattern_without_a_base_records_its_base_mode() -> Result<()> {
    let pattern = "glob-diagnostics-relative-without-base/no-such-dir/*.txt";
    let cache = GlobBaseCache::new(None);

    let (results, _events, snapshot) = recorded(|| -> Result<Vec<String>> {
        let expansion = expand_manifest_template_glob(pattern, &cache)?;
        Ok(GlobExpansion::into_paths(expansion))
    });
    ensure!(
        results?.is_empty(),
        "the missing prefix should match nothing"
    );
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[
                ("base_mode", "relative_without_base"),
                ("outcome", "unopenable_prefix"),
            ]
        ) == Some(1),
        "the template boundary should classify the unbased relative pattern: {snapshot:?}"
    );
    Ok(())
}

/// Verify that injected-base preparation failures still complete telemetry.
#[rstest]
fn an_unresolvable_injected_base_records_one_terminal_failure() -> Result<()> {
    let temp = tempdir()?;
    let base = Utf8Path::from_path(temp.path())
        .context("temporary directory should have a UTF-8 path")?
        .join("missing");
    let cache = GlobBaseCache::new(Some(base));

    let (result, _events, snapshot) = recorded(|| expand_manifest_template_glob("*.txt", &cache));
    ensure!(
        result.is_err(),
        "a missing injected base must fail expansion"
    );
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[
                ("base_mode", "relative_with_base"),
                ("outcome", "base_canonicalization_failure"),
            ]
        ) == Some(1),
        "the preparation failure should record one terminal counter: {snapshot:?}"
    );
    ensure!(
        has_histogram(&snapshot, TEMPLATE_EXPANSION_DURATION),
        "the preparation failure should record one duration: {snapshot:?}"
    );
    Ok(())
}

/// Verify that a symlinked literal prefix reports a capability-root failure.
#[cfg(unix)]
#[rstest]
fn a_symlinked_literal_prefix_records_a_capability_root_failure() -> Result<()> {
    let temp = tempdir()?;
    let target = temp.path().join("target");
    test_fs::create_dir(&target)?;
    test_fs::write(target.join("a.txt"), "a")?;
    test_fs::symlink("target", temp.path().join("link"))?;
    let pattern = format!("{}/link/*.txt", temp.path().display());
    let cache = GlobBaseCache::new(None);

    let (result, events, snapshot) = recorded(|| expand_manifest_template_glob(&pattern, &cache));
    ensure!(
        result.is_err(),
        "a symlinked literal prefix must fail expansion"
    );
    ensure!(
        counter_value_with_labels(
            &snapshot,
            TEMPLATE_EXPANSIONS,
            &[
                ("base_mode", "absolute_pattern"),
                ("outcome", "capability_root_io_failure"),
            ]
        ) == Some(1),
        "the template boundary should classify the capability failure: {snapshot:?}"
    );
    ensure!(
        events.iter().any(|event| {
            event.contains("operation=\"manifest_template_glob_expansion\"")
                && event.contains("outcome=\"capability_root_io_failure\"")
                && event.contains("manifest template glob expansion failed")
        }),
        "the template boundary should trace the bounded capability failure: {events:?}"
    );
    ensure!(
        !events
            .iter()
            .any(|event| event.contains(&temp.path().display().to_string())),
        "capability failure events must not disclose the literal prefix: {events:?}"
    );
    Ok(())
}
