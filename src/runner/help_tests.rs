//! Unit snapshot tests for the `netsuke help targets` renderer.
//!
//! The fixture manifest mirrors the issue's suggested shape: actions and
//! targets with descriptions, manifest defaults, and one entry whose
//! description is missing so the empty-column representation is pinned.

use super::*;
use crate::ast::{NetsukeManifest, Target};
use crate::cli_localization::build_localizer;
use crate::localization::set_localizer_for_tests;
use crate::manifest;
use crate::snapshot_test_support::{snapshot_settings, theme_prefs};
use crate::theme::ThemePreference;
use anyhow::{Context, Result};
use insta::assert_snapshot;
use proptest::prelude::*;
use semver::Version;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;
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
    let manifest = fixture_manifest()?;
    let rendered = render_catalogue_with_locale(locale, &manifest, render)?;
    snapshot_settings("help_targets").bind(|| {
        assert_snapshot!(snapshot_name, rendered);
    });
    Ok(())
}

/// Render a catalogue while holding the localizer lock only for its global
/// localization dependency.
fn render_catalogue_with_locale(
    locale: &str,
    manifest: &NetsukeManifest,
    render: impl FnOnce(&NetsukeManifest) -> Result<String>,
) -> Result<String> {
    let _lock = localizer_lock();
    let _guard = set_localizer_for_tests(Arc::from(build_localizer(Some(locale))));
    render(manifest)
}

#[test]
fn catalogue_rendering_releases_localizer_lock_before_snapshot_work() -> Result<()> {
    let manifest = fixture_manifest()?;
    let rendered = render_catalogue_with_locale("en-US", &manifest, |parsed_manifest| {
        render_json(&build_catalogue(parsed_manifest))
    })?;
    let (acquired, confirmed) = mpsc::sync_channel(0);
    let contender = thread::spawn(move || {
        let _lock = localizer_lock();
        acquired.send(()).ok();
    });
    confirmed
        .recv_timeout(Duration::from_secs(5))
        .context("localizer contender should acquire the lock before snapshot work")?;
    contender
        .join()
        .map_err(|_| anyhow::anyhow!("localizer contender should complete"))?;
    anyhow::ensure!(
        rendered.contains("\"command\": \"help-targets\""),
        "rendered catalogue should remain available after localizer contention"
    );
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
        render_json(&build_catalogue(manifest))
    })
}

#[test]
fn text_catalogue_escapes_terminal_control_characters() -> Result<()> {
    let mut manifest = fixture_manifest()?;
    let action = manifest
        .actions
        .first_mut()
        .context("help target fixture should contain an action")?;
    action.name = crate::ast::StringOrList::String(
        "line\nnext\t\u{001B}[31mred\u{009B}m\u{202E}reordered".to_owned(),
    );
    action.description = Some("description\r\nwith\tcontrols\u{0007}\u{202E}".to_owned());

    let output = render_text(
        &build_catalogue(&manifest),
        theme_prefs(ThemePreference::Unicode),
    );

    anyhow::ensure!(
        output.contains("line\\nnext\\t\\u{1b}[31mred\\u{9b}m\\u{202e}reordered"),
        "name controls should be visible escapes: {output:?}"
    );
    anyhow::ensure!(
        output.contains("description\\r\\nwith\\tcontrols\\u{7}\\u{202e}"),
        "description controls should be visible escapes: {output:?}"
    );
    anyhow::ensure!(
        !output.contains('\r')
            && !output.contains('\u{001B}')
            && !output.contains('\u{009B}')
            && !output.contains('\u{202E}'),
        "text output must not contain terminal control characters: {output:?}"
    );
    anyhow::ensure!(
        output.lines().count() == 8,
        "escaped newlines must not create additional catalogue rows: {output:?}"
    );
    Ok(())
}

#[test]
fn text_catalogue_escapes_cc_range_boundaries() -> Result<()> {
    let mut manifest = fixture_manifest()?;
    let action = manifest
        .actions
        .first_mut()
        .context("help target fixture should contain an action")?;
    action.name = crate::ast::StringOrList::String(
        "start\0unit\u{001F}delete\u{007F}application\u{009F}end".to_owned(),
    );

    let output = render_text(
        &build_catalogue(&manifest),
        theme_prefs(ThemePreference::Unicode),
    );

    for escaped in ["\\u{0}", "\\u{1f}", "\\u{7f}", "\\u{9f}"] {
        anyhow::ensure!(
            output.contains(escaped),
            "text catalogue should escape Cc boundary {escaped}: {output:?}"
        );
    }
    for control in ['\0', '\u{001F}', '\u{007F}', '\u{009F}'] {
        anyhow::ensure!(
            !output.contains(control),
            "text catalogue must not contain Cc boundary {control:?}: {output:?}"
        );
    }
    Ok(())
}

/// Generate target metadata with at least one name, allowing actions and
/// targets to exercise scalar/list flattening through the same catalogue path.
fn target_metadata() -> impl Strategy<Value = (Vec<String>, Option<String>)> {
    (
        proptest::collection::vec("[a-z]{1,8}", 1..4),
        prop_oneof![Just(None), "[A-Za-z ]{0,20}".prop_map(Some)],
    )
}

/// Build a simple target because catalogue construction depends only on names,
/// descriptions, and action categorization.
fn catalogue_target(names: Vec<String>, description: Option<String>, phony: bool) -> Target {
    Target {
        name: crate::ast::StringOrList::List(names),
        recipe: crate::ast::Recipe::Command {
            command: crate::ast::StringOrList::String("true".to_owned()),
        },
        sources: crate::ast::StringOrList::Empty,
        deps: crate::ast::StringOrList::Empty,
        order_only_deps: crate::ast::StringOrList::Empty,
        vars: crate::ast::Vars::default(),
        phony,
        always: false,
        description,
    }
}

proptest! {
    /// Catalogue construction preserves declaration order, expands every name,
    /// retains metadata, and marks each alias selected by `defaults`.
    #[test]
    fn catalogue_preserves_order_names_metadata_and_defaults(
        actions in proptest::collection::vec(target_metadata(), 0..5),
        targets in proptest::collection::vec(target_metadata(), 0..5),
        default_flags in proptest::collection::vec(any::<bool>(), 0..64),
    ) {
        let declared_names: Vec<String> = actions
            .iter()
            .chain(&targets)
            .flat_map(|(names, _)| names.iter().cloned())
            .collect();
        let defaults = if declared_names.is_empty() {
            Vec::new()
        } else {
            declared_names
                .iter()
                .zip(default_flags)
                .filter(|(_, is_default)| *is_default)
                .map(|(name, _)| name.clone())
                .collect()
        };
        let manifest = NetsukeManifest {
            netsuke_version: Version::new(1, 0, 0),
            vars: crate::ast::Vars::default(),
            macros: Vec::new(),
            rules: Vec::new(),
            actions: actions
                .iter()
                .cloned()
                .map(|(names, description)| catalogue_target(names, description, true))
                .collect(),
            targets: targets
                .iter()
                .cloned()
                .map(|(names, description)| catalogue_target(names, description, false))
                .collect(),
            defaults: defaults.clone(),
        };
        let default_names = &defaults;
        let expected: Vec<(String, Option<String>, bool, bool)> = actions
            .iter()
            .map(|(names, description)| (names, description, true))
            .chain(targets.iter().map(|(names, description)| (names, description, false)))
            .flat_map(|(names, description, is_action)| {
                names.iter().cloned().map(move |name| {
                    let is_default = default_names.contains(&name);
                    (name, description.clone(), is_action, is_default)
                })
            })
            .collect();
        let actual: Vec<(String, Option<String>, bool, bool)> = build_catalogue(&manifest)
            .into_iter()
            .map(|entry| (
                entry.name,
                entry.description.as_deref().map(str::to_owned),
                entry.is_action,
                entry.is_default,
            ))
            .collect();

        prop_assert_eq!(actual, expected);
    }
}
