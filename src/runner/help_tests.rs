//! Unit snapshot tests for the `netsuke help targets` renderer.
//!
//! The fixture manifest mirrors the issue's suggested shape: actions and
//! targets with descriptions, manifest defaults, and one entry whose
//! description is missing so the empty-column representation is pinned.

use super::*;
use crate::cli_localization::build_localizer;
use crate::localization::set_localizer_for_tests;
use crate::manifest;
use crate::snapshot_test_support::{snapshot_settings, theme_prefs};
use crate::theme::ThemePreference;
use anyhow::Result;
use insta::assert_snapshot;
use std::sync::Arc;
use test_support::fluent::normalize_fluent_isolates;
use test_support::localizer_test_lock;

/// Parse the fixed fixture manifest used by the catalogue snapshots.
fn fixture_manifest() -> Result<NetsukeManifest> {
    let yaml = r#"netsuke_version: "1.0.0"
actions:
  - name: lint
    description: Run rustdoc, Clippy, and Whitaker
    command: cargo clippy --all-targets --all-features -- -D warnings
  - name: test
    description: Run unit, behavioural, UI, and documentation tests
    command: cargo test
  - name: undocumented
    command: echo hi
targets:
  - name: target/release/catnap
    description: Build the optimized release binary
    command: cargo build --release
  - name: plain
    command: echo plain
defaults:
  - lint
  - test
"#;
    manifest::from_str(yaml)
}

/// Acquire the localizer test lock, recovering from poisoning the way the
/// test-support fixtures do, so one failing snapshot cannot cascade into the
/// tests that follow it.
fn localizer_lock() -> std::sync::MutexGuard<'static, ()> {
    localizer_test_lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Run one catalogue snapshot: install the locale, render through the closure,
/// and bind the snapshot assertion.
///
/// The assertion name is passed at runtime, so all four snapshot tests share
/// this setup while keeping their distinct snapshot files.
fn catalogue_snapshot(
    locale: &str,
    snapshot_name: &str,
    render: impl FnOnce(&NetsukeManifest) -> Result<String>,
) -> Result<()> {
    let _lock = localizer_lock();
    let _guard = set_localizer_for_tests(Arc::from(build_localizer(Some(locale))));
    let manifest = fixture_manifest()?;
    let rendered = render(&manifest)?;
    snapshot_settings("help_targets").bind(|| {
        assert_snapshot!(snapshot_name, rendered);
    });
    Ok(())
}

#[test]
fn text_catalogue_snapshot() -> Result<()> {
    catalogue_snapshot("en-US", "text_catalogue", |manifest| {
        Ok(normalize_fluent_isolates(&render_text(
            &build_catalogue(manifest),
            theme_prefs(ThemePreference::Unicode),
        )))
    })
}

#[test]
fn accessible_catalogue_snapshot() -> Result<()> {
    catalogue_snapshot("en-US", "accessible_catalogue", |manifest| {
        Ok(normalize_fluent_isolates(&render_text(
            &build_catalogue(manifest),
            theme_prefs(ThemePreference::Ascii),
        )))
    })
}

#[test]
fn localized_catalogue_snapshot() -> Result<()> {
    catalogue_snapshot("es-ES", "localized_catalogue_es_es", |manifest| {
        Ok(normalize_fluent_isolates(&render_text(
            &build_catalogue(manifest),
            theme_prefs(ThemePreference::Unicode),
        )))
    })
}

#[test]
fn json_catalogue_snapshot() -> Result<()> {
    catalogue_snapshot("en-US", "json_catalogue", |manifest| {
        render_json(build_catalogue(manifest))
    })
}
