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

/// Resolve explicit-theme preferences for deterministic snapshot tests.
pub(crate) fn theme_prefs(theme: ThemePreference) -> OutputPrefs {
    resolve_from_theme_with(
        Some(theme),
        ThemeContext::new(None, None, OutputMode::Standard),
        |_| None,
    )
}
