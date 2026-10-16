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

/// Build snapshot settings rooted at `src/snapshots/<subdir>` with the module
/// path left out of the snapshot name.
///
/// [`snapshot_settings`] keeps insta's module-prefixed names, which is right for
/// a suite whose modules are settled. It is wrong for a suite that a later
/// milestone moves between modules: the prefix changes with the module, so the
/// snapshot would be renamed — and a rename during a pure move reads as a
/// behaviour change. Removing the prefix makes the name depend only on the
/// assertion, so the rows survive being relocated.
///
/// Both of a snapshot's incidental inputs are neutralized here, because both
/// would otherwise vary without the code under test varying: the module path
/// names the file, and a temporary directory reaches the text whenever a row
/// stages a workspace and an error names it.
///
/// Callers pass a directory unique to their suite, because two rows in
/// different modules sharing one name would silently overwrite each other.
pub(crate) fn stable_snapshot_settings(subdir: &str) -> Settings {
    let mut settings = snapshot_settings(subdir);
    settings.set_prepend_module_to_snapshot(false);
    add_temp_dir_filter(&mut settings);
    settings
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
/// the same characters alone: `/tmp` would otherwise rewrite an unrelated
/// `/tmpfoo/` component in a path or in a diagnostic.
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
            other => regex::escape(&other.to_string()),
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

/// Add a redaction filter for the Netsuke generator version.
///
/// Anchor the filter on the enclosing generator object so unrelated versioned
/// content remains visible in snapshot diffs.
fn add_generator_version_filter(settings: &mut Settings) {
    settings.add_filter(
        GENERATOR_VERSION_FILTER_PATTERN,
        GENERATOR_VERSION_REPLACEMENT,
    );
}

/// Match the generator-version field scoped to Netsuke's generator object.
const GENERATOR_VERSION_FILTER_PATTERN: &str =
    r#"("generator": \{\s*\n\s*"name": "netsuke",\s*\n\s*"version": ")[^"]+(")"#;

/// Replace the matched generator version while retaining its JSON structure.
const GENERATOR_VERSION_REPLACEMENT: &str = r"${1}[version]${2}";

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

#[cfg(test)]
mod tests {
    //! Verify the two snapshot filters redact exactly what they claim.
    //!
    //! Both filters are the reason a checked-in snapshot repeats on a machine
    //! that did not record it: one hides the generator version across `SemVer`
    //! values, the other hides the temporary directory a staged workspace lives
    //! in.

    use super::*;
    use anyhow::{Result, ensure};
    use proptest::prelude::*;
    use regex::Regex;
    use rstest::rstest;
    use serde_json::Value;

    /// Replace every match of the temporary-path filter with `[TMP]`.
    fn normalized(temp_dir: &str, text: &str) -> Result<String> {
        let pattern = temp_dir_filter_pattern(temp_dir);
        let filter = Regex::new(&pattern)?;
        Ok(filter.replace_all(text, "[TMP]").into_owned())
    }

    #[rstest]
    #[case(
        r"C:\Users\runner\AppData\Local\Temp",
        r"cannot read C:\Users\runner\AppData\Local\Temp\ws\Netsukefile",
        "cannot read [TMP]",
        "a diagnostic quotes a Windows path with single backslashes"
    )]
    #[case(
        r"C:\Users\runner\AppData\Local\Temp",
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

    proptest! {
        #[test]
        fn generator_version_filter_redacts_semver_variants(
            major in any::<u64>(),
            minor in any::<u64>(),
            patch in any::<u64>(),
            prerelease in prop_oneof![
                Just(String::new()),
                "[A-Za-z][A-Za-z0-9-]{0,31}".prop_map(|identifier| format!("-{identifier}")),
            ],
            build in prop_oneof![
                Just(String::new()),
                "[A-Za-z][A-Za-z0-9-]{0,31}".prop_map(|identifier| format!("+{identifier}")),
            ],
            unrelated_version in "[A-Za-z0-9.+-]{1,64}",
        ) {
            let generator_version = format!("{major}.{minor}.{patch}{prerelease}{build}");
            let rendered = format!(
                concat!(
                    "{{\n",
                    "  \"generator\": {{\n",
                    "    \"name\": \"netsuke\",\n",
                    "    \"version\": \"{}\"\n",
                    "  }},\n",
                    "  \"tool\": {{\n",
                    "    \"name\": \"netsuke\",\n",
                    "    \"version\": \"{}\"\n",
                    "  }}\n",
                    "}}",
                ),
                generator_version,
                unrelated_version,
            );
            let filter = match Regex::new(GENERATOR_VERSION_FILTER_PATTERN) {
                Ok(filter) => filter,
                Err(error) => return Err(TestCaseError::fail(error.to_string())),
            };
            let filtered = filter.replace_all(&rendered, GENERATOR_VERSION_REPLACEMENT);
            let document = match serde_json::from_str::<Value>(&filtered) {
                Ok(document) => document,
                Err(error) => return Err(TestCaseError::fail(error.to_string())),
            };

            prop_assert_eq!(
                document.pointer("/generator/version").and_then(Value::as_str),
                Some("[version]"),
            );
            prop_assert_eq!(
                document.pointer("/tool/version").and_then(Value::as_str),
                Some(unrelated_version.as_str()),
            );
        }
    }
}
