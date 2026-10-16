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

/// Build the pattern that replaces a temporary directory with `[TMP]`.
///
/// Each separator becomes a class matching one or more of either separator, and
/// every other character is escaped. The class is what lets one pattern cover
/// both spellings a snapshot can hold: a diagnostic quotes a Windows path with
/// the single backslashes `temp_dir()` renders, while the serialized half of the
/// same snapshot doubles them, so a pattern requiring exactly one separator
/// would leave the serialized copy unnormalized — and it is the serialized copy
/// that carries the run-specific workspace name. A trailing separator is
/// dropped first, because leaving it in would make two adjacent classes demand
/// two separators where the text has one.
///
/// Requiring a whole component at each step, rather than matching the directory
/// name as a bare substring, is what leaves a directory that merely starts with
/// the same characters alone: `/tmp` is a short enough string that an unescaped,
/// unanchored pattern would happily rewrite an unrelated `/tmpfoo/` component
/// in a path or in a diagnostic.
///
/// The placeholder is the same on every platform, which matters because the
/// snapshots are checked in: a row that records only `[TMP]` reads the same
/// whichever machine recorded it.
fn temp_dir_filter_pattern(temp_dir: &str) -> String {
    let separator = r"[\\/]+";
    let prefix = temp_dir
        .trim_end_matches(['/', '\\'])
        .chars()
        .map(|ch| match ch {
            '/' | '\\' => separator.to_owned(),
            other => escape(&other.to_string()),
        })
        .collect::<String>();
    format!("{prefix}{separator}[^\\s\"']*")
}

/// Replace the temporary-directory prefix of any absolute path with a
/// placeholder.
///
/// Resolving the directory here rather than in [`temp_dir_filter_pattern`] keeps
/// the pattern a pure function of its input, so a test can assert the Windows
/// spelling on any host.
fn add_temp_dir_filter(settings: &mut Settings) {
    let temp_dir = std::env::temp_dir().display().to_string();
    settings.add_filter(&temp_dir_filter_pattern(&temp_dir), "[TMP]");
}

#[cfg(test)]
mod tests {
    //! Assert the temporary-path filter covers both spellings of one path.
    //!
    //! The Windows rows run on every platform because
    //! [`temp_dir_filter_pattern`] takes its directory as an argument. That
    //! matters: the defect these rows pin is invisible on Linux, so a test that
    //! asked `temp_dir()` for the prefix could only ever guard the half of the
    //! problem the recording host happens to have.

    use super::temp_dir_filter_pattern;
    use anyhow::{Result, ensure};
    use regex::Regex;
    use rstest::rstest;

    /// Replace every match of the filter for `temp_dir` with `[TMP]`.
    fn normalized(temp_dir: &str, text: &str) -> Result<String> {
        let pattern = temp_dir_filter_pattern(temp_dir);
        let filter = Regex::new(&pattern)?;
        Ok(filter.replace_all(text, "[TMP]").into_owned())
    }

    const WINDOWS_TEMP: &str = r"C:\Users\runner\AppData\Local\Temp";

    #[rstest]
    #[case(
        WINDOWS_TEMP,
        r"cannot read C:\Users\runner\AppData\Local\Temp\ws\Netsukefile",
        "cannot read [TMP]",
        "a diagnostic quotes a Windows path with single backslashes"
    )]
    #[case(
        WINDOWS_TEMP,
        r#"{"path": "C:\\Users\\runner\\AppData\\Local\\Temp\\ws"}"#,
        r#"{"path": "[TMP]"}"#,
        "serializing a Windows path doubles every backslash"
    )]
    #[case("/tmp", "/tmp/ws/Netsukefile", "[TMP]", "a Unix path")]
    #[case("/tmp/", "/tmp/ws", "[TMP]", "a trailing separator changes nothing")]
    #[case("/tmp", "/tmpfoo/ws", "/tmpfoo/ws", "a sibling sharing the prefix")]
    fn temp_dir_filter_replaces_exactly_the_temporary_prefix(
        #[case] temp_dir: &str,
        #[case] text: &str,
        #[case] expected: &str,
        #[case] because: &str,
    ) -> Result<()> {
        let actual = normalized(temp_dir, text)?;
        ensure!(
            actual == expected,
            "{because}: expected {expected:?}, got {actual:?}"
        );
        Ok(())
    }
}
