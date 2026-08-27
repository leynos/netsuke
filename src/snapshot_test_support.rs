//! Shared helpers for output-oriented unit tests.
//!
//! These helpers keep theme-based `OutputPrefs` resolution and snapshot-path
//! setup consistent across multiple test modules.

use insta::Settings;
use rstest::fixture;
use std::path::PathBuf;

use crate::output_mode::OutputMode;
use crate::output_prefs::{OutputPrefs, resolve_from_theme_with};
use crate::theme::{ThemeContext, ThemePreference};

/// Environment lookup used by tests exercising optional `NO_COLOR` handling.
pub(crate) type NoColorEnv = fn(Option<String>, &str) -> Option<String>;

/// Provide a shared lookup for an optional `NO_COLOR` value.
#[fixture]
pub(crate) fn no_color_env() -> NoColorEnv {
    |no_color, key| match key {
        "NO_COLOR" => no_color,
        _ => None,
    }
}

/// Build snapshot settings rooted at `src/snapshots/<subdir>`.
pub(crate) fn snapshot_settings(subdir: &str) -> Settings {
    let mut settings = Settings::new();
    settings.set_snapshot_path(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("snapshots")
            .join(subdir),
    );
    settings
}

/// Add a redaction filter for the Netsuke generator version.
///
/// Anchor the filter on the enclosing generator object so unrelated versioned
/// content remains visible in snapshot diffs.
fn add_generator_version_filter(settings: &mut Settings) {
    settings.add_filter(
        r#"("generator": \{\s*\n\s*"name": "netsuke",\s*\n\s*"version": ")[^"]+(")"#,
        r"${1}[version]${2}",
    );
}

/// Build snapshot settings for diagnostic-JSON documents.
///
/// Extends [`snapshot_settings`] with a redaction filter for the generator's
/// version, so snapshots survive version bumps. The filter anchors on the
/// enclosing `"generator"` object and its `"name": "netsuke"` line; any other
/// `version` field in a rendered document stays visible in snapshot diffs.
/// Diagnostic-JSON snapshot tests in any module must bind through this helper
/// so the redaction is applied consistently.
pub(crate) fn diagnostic_json_snapshot_settings() -> Settings {
    let mut settings = snapshot_settings("diagnostic_json");
    add_generator_version_filter(&mut settings);
    settings
}

/// Build snapshot settings for JSON help-target catalogues.
///
/// Extend the help-target settings with the generator-version redaction while
/// retaining unfiltered settings for text catalogues.
pub(crate) fn help_targets_json_snapshot_settings() -> Settings {
    let mut settings = snapshot_settings("help_targets");
    add_generator_version_filter(&mut settings);
    settings
}

/// Resolve explicit-theme preferences for deterministic snapshot tests.
pub(crate) fn theme_prefs(theme: ThemePreference) -> OutputPrefs {
    resolve_from_theme_with(
        Some(theme),
        ThemeContext::new(None, None, OutputMode::Standard),
        |_| None,
    )
}
