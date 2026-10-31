//! Insta settings for the characterization snapshots.
//!
//! The settings exist to make one property true: a snapshot file's name and
//! directory depend only on the row that produced it, never on which module the
//! row happens to live in. Insta's defaults are the opposite — the name gains
//! the module path and the directory follows the assertion file — so a routine
//! refactor would move every snapshot and read as a behaviour change. Both
//! defaults are switched off here and replaced with one fixed directory.
//!
//! One filter stays on: a path entry point loads its fixture from a workspace it
//! creates, and an error names the manifest by the absolute path it was given,
//! so the temporary prefix has to be replaced for that text to repeat.

use camino::{Utf8Path, Utf8PathBuf};
use insta::Settings;
use test_support::temp_path_filter::temp_dir_filter_pattern;

/// Return the repository root, resolved at compile time.
pub fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Return the directory holding the public suite's snapshots.
pub fn public_snapshot_dir() -> Utf8PathBuf {
    repo_root().join("tests/snapshots/manifest_load_options")
}

/// Build settings that pin snapshot paths and hide temporary directories.
///
/// `snapshot_dir` must be absolute. Insta resolves a relative path against the
/// directory of the file containing the assertion, so a relative path would
/// reintroduce exactly the module-dependence the absolute path removes.
pub fn settings_in(snapshot_dir: &Utf8Path) -> Settings {
    let mut settings = Settings::new();
    settings.set_snapshot_path(snapshot_dir);
    settings.set_prepend_module_to_snapshot(false);
    add_temp_dir_filter(&mut settings);
    settings
}

/// Return the public suite's settings.
pub fn settings() -> Settings {
    settings_in(&public_snapshot_dir())
}

/// Replace the temporary-directory prefix of any absolute path with a
/// placeholder.
///
/// Resolving the directory here rather than in [`temp_dir_filter_pattern`] keeps
/// the pattern a pure function of its input, so its tests can assert the
/// Windows spelling on any host.
fn add_temp_dir_filter(settings: &mut Settings) {
    let temp_dir = std::env::temp_dir().display().to_string();
    settings.add_filter(&temp_dir_filter_pattern(&temp_dir), "[TMP]");
}
