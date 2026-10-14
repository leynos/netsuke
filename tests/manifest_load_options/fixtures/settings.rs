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
//! creates, and the loader puts that workspace's root into the manifest it
//! returns, so the temporary prefix has to be replaced for the snapshot to
//! repeat.

use camino::{Utf8Path, Utf8PathBuf};
use insta::Settings;
use regex::escape;

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
/// The prefix is escaped before it becomes a pattern, and the pattern then
/// requires a separator, so a directory that merely starts with the same
/// characters is left alone. `/tmp` is a short enough string that an unescaped,
/// unanchored pattern would happily rewrite an unrelated `/tmpfoo/` component
/// in a path or in a diagnostic.
///
/// The placeholder is the same on every platform, which matters because the
/// snapshots are checked in: a row that records only `[TMP]` reads the same
/// whichever machine recorded it.
fn add_temp_dir_filter(settings: &mut Settings) {
    let prefix = escape(&std::env::temp_dir().display().to_string());
    settings.add_filter(&format!(r#"{prefix}/[^\s"']*"#), "[TMP]");
}
