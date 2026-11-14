//! Shared helpers for output-oriented unit tests.
//!
//! These helpers keep theme-based `OutputPrefs` resolution and snapshot-path
//! setup consistent across multiple test modules.

use insta::Settings;
use rstest::fixture;
use std::path::PathBuf;
use test_support::temp_path_filter::temp_dir_filter_pattern;

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

/// Replace the temporary-directory prefix of any absolute path with a
/// placeholder.
///
/// Resolving the directory here rather than in
/// [`temp_dir_filter_pattern`] keeps the pattern a pure function of its input,
/// so a test can assert the Windows spelling on any host.
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

/// Add redaction filters for a schema-admission diagnostic snapshot.
///
/// The declared and supported schema releases, and the manifest file name that
/// appears in the sentence, are sample values rather than properties of the
/// diagnostic shape. Leaving them literal makes the row fail for the wrong
/// reason twice over: it changes whenever `SupportedSchema::max_minor` advances,
/// and it records a version pair that only one fixture supplies. Redacting them
/// leaves the sentence's structure and code visible, which is what the snapshot
/// exists to hold.
///
/// The patterns are anchored on the sentence shape the localizer emits, so an
/// unrelated version or file name elsewhere in the document stays visible.
pub(crate) fn schema_admission_diagnostic_snapshot_settings() -> Settings {
    let mut settings = diagnostic_json_snapshot_settings();
    settings.add_filter(DECLARED_SCHEMA_FILTER_PATTERN, DECLARED_SCHEMA_REPLACEMENT);
    settings.add_filter(
        SUPPORTED_SCHEMA_FILTER_PATTERN,
        SUPPORTED_SCHEMA_REPLACEMENT,
    );
    settings.add_filter(MANIFEST_NAME_FILTER_PATTERN, MANIFEST_NAME_REPLACEMENT);
    settings
}

/// Match the declared schema release in a schema-admission cause sentence.
///
/// Each pattern captures the surrounding wording and replaces only the
/// substituted value, so the stored snapshot keeps reading as a sentence.
const DECLARED_SCHEMA_FILTER_PATTERN: &str = r"(manifest schema )\x{2068}[0-9]+\.[0-9]+\x{2069}";

/// Replace the declared release, keeping the clause that introduces it.
const DECLARED_SCHEMA_REPLACEMENT: &str = "${1}[declared_schema]";

/// Match the supported schema release in a schema-admission cause sentence.
const SUPPORTED_SCHEMA_FILTER_PATTERN: &str = r"(supports schema )\x{2068}[0-9]+\.[0-9]+\x{2069}";

/// Replace the supported release, keeping the clause that introduces it.
const SUPPORTED_SCHEMA_REPLACEMENT: &str = "${1}[supported_schema]";

/// Match the manifest file name in a schema-admission cause sentence.
///
/// Fluent isolates a substituted argument between U+2068 (first strong isolate)
/// and U+2069 (pop directional isolate). Consuming the marks with the value
/// matters twice over: a leftover isolate is invisible in the stored snapshot,
/// and the name is only identifiable because the clause that follows it is
/// fixed.
const MANIFEST_NAME_FILTER_PATTERN: &str =
    r"\x{2068}[^\x{2069}]*\x{2069}( declares manifest schema)";

/// Replace the captured file name, keeping the clause that identifies it.
const MANIFEST_NAME_REPLACEMENT: &str = "[manifest]${1}";

/// Build snapshot settings for JSON help-target catalogues.
///
/// Extend the help-target settings with the generator-version redaction while
/// retaining unfiltered settings for text catalogues.
pub(crate) fn help_targets_json_snapshot_settings() -> Settings {
    let mut settings = snapshot_settings("help_targets");
    add_generator_version_filter(&mut settings);
    settings
}

/// Build snapshot settings for `netsuke check` result documents.
///
/// Extend the check settings with the generator-version redaction so lint
/// snapshots survive version bumps, on the same terms as every other JSON
/// document snapshot.
#[cfg(feature = "lint")]
pub(crate) fn check_json_snapshot_settings() -> Settings {
    let mut settings = snapshot_settings("check");
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
    //! Verify the generator-version filter redacts exactly what it claims.
    //!
    //! Both snapshot filters are the reason a checked-in snapshot repeats on a
    //! machine that did not record it: one hides the generator version across
    //! `SemVer` values, the other hides the temporary directory a staged
    //! workspace lives in.
    //!
    //! The version and schema-admission filters are exercised here, because
    //! this module owns them. The temporary-path pattern moved to
    //! `test_support::temp_path_filter` and its cases moved with it, so this
    //! crate and the integration suite share one set of rows rather than
    //! holding a copy each.

    use super::*;
    use anyhow::{Result, ensure};
    use proptest::prelude::*;
    use regex::Regex;
    use rstest::rstest;
    use serde_json::Value;

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

    /// Redact one schema-admission cause sentence the way the settings do.
    fn redact_schema_admission(sentence: &str) -> Result<String, regex::Error> {
        let declared = Regex::new(DECLARED_SCHEMA_FILTER_PATTERN)?;
        let supported = Regex::new(SUPPORTED_SCHEMA_FILTER_PATTERN)?;
        let name = Regex::new(MANIFEST_NAME_FILTER_PATTERN)?;
        let without_declared = declared.replace_all(sentence, DECLARED_SCHEMA_REPLACEMENT);
        let without_supported =
            supported.replace_all(&without_declared, SUPPORTED_SCHEMA_REPLACEMENT);
        Ok(name
            .replace_all(&without_supported, MANIFEST_NAME_REPLACEMENT)
            .into_owned())
    }

    /// Wrap an argument the way Fluent isolates a substituted value.
    ///
    /// The marks are built from escapes rather than written literally, because
    /// rustc denies invisible text-direction codepoints in source literals.
    fn isolated(value: &str) -> String {
        format!("\u{2068}{value}\u{2069}")
    }

    /// The schema-admission filters must keep the sentence readable while
    /// removing every fixture value, because a filter that stops matching fails
    /// silently: the snapshot simply starts recording the sample data again.
    #[rstest]
    #[case::supported_1_0("Netsukefile", "1.1", "1.0")]
    #[case::later_minor("build.ninja", "99.42", "1.7")]
    fn schema_admission_filters_redact_fixture_values(
        #[case] name: &str,
        #[case] declared: &str,
        #[case] supported: &str,
    ) -> Result<()> {
        let sentence = format!(
            "{} declares manifest schema {}, but this Netsuke supports schema {}.",
            isolated(name),
            isolated(declared),
            isolated(supported),
        );
        let redacted = redact_schema_admission(&sentence)?;
        let expected = concat!(
            "[manifest] declares manifest schema [declared_schema], ",
            "but this Netsuke supports schema [supported_schema]."
        );
        ensure!(redacted == expected, "unexpected redaction: {redacted:?}");
        ensure!(
            !redacted.contains('\u{2068}') && !redacted.contains('\u{2069}'),
            "directional isolates must be consumed with the values they wrap: {redacted:?}"
        );
        Ok(())
    }
}
