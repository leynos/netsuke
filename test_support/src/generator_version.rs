//! The diagnostic-JSON generator-version filter shared by both snapshot trees.
//!
//! Diagnostic-JSON snapshots quote the crate version inside the enclosing
//! generator object, so an unchanged document would otherwise look like drift
//! after every release. Redacting only that field is what keeps the snapshot
//! sensitive to everything else: a version-shaped string anywhere else in the
//! document — another object's `version`, or a version quoted in a message —
//! stays visible and must still be recorded.
//!
//! The pattern lives here because the two callers cannot reach each other: the
//! unit-test settings in `src/snapshot_test_support.rs` are `#[cfg(test)]` code
//! inside the library, while the startup suite under
//! `tests/cli_tests/shells_startup/` is a separate integration crate that links
//! the library's public surface only. Two copies could drift apart and still
//! pass, because each would be guarded only by its own cases; one definition
//! makes the anchor a single reviewed decision.
//!
//! # Examples
//!
//! ```rust
//! use test_support::generator_version::redact_generator_version;
//!
//! let document = "{\n  \"generator\": {\n    \"name\": \"netsuke\",\n    \
//!                 \"version\": \"0.1.0\"\n  }\n}";
//! let redacted = redact_generator_version(document)?;
//! assert!(redacted.contains("\"version\": \"[version]\""));
//! # Ok::<(), anyhow::Error>(())
//! ```

use anyhow::{Context, Result};
use regex::Regex;

/// Match the generator object's version value, and nothing else.
///
/// The anchor is the enclosing `"generator"` object and its `"name": "netsuke"`
/// line, so a document that quotes the same version elsewhere keeps it in the
/// snapshot. Whitespace between the object's fields is allowed because the
/// serialiser decides the layout, not this pattern.
pub const GENERATOR_VERSION_PATTERN: &str =
    r#"("generator": \{\s*\n\s*"name": "netsuke",\s*\n\s*"version": ")[^"]+(")"#;

/// Replacement for `GENERATOR_VERSION_PATTERN`: keep the framing, redact the
/// value.
pub const GENERATOR_VERSION_REPLACEMENT: &str = r"${1}[version]${2}";

/// Replace the generator object's version with `[version]`.
///
/// Text outside that object passes through unchanged, including a version that
/// a diagnostic quotes in its message. Redacting the whole document by value
/// would be simpler, but it would also hide exactly the class of change a
/// snapshot is meant to catch.
///
/// # Errors
///
/// Returns the regex compilation error if the shared pattern is malformed,
/// which is a defect in this crate rather than in the caller's input.
pub fn redact_generator_version(text: &str) -> Result<String> {
    let pattern = Regex::new(GENERATOR_VERSION_PATTERN)
        .context("compile the generator-version filter pattern")?;
    Ok(pattern
        .replace_all(text, GENERATOR_VERSION_REPLACEMENT)
        .into_owned())
}

#[cfg(test)]
mod tests {
    //! Assert the filter redacts the generator version and nothing else.
    //!
    //! Both snapshot filters are the reason a checked-in snapshot repeats on a
    //! machine that did not record it. The temporary-path filter's rows live in
    //! `temp_path_filter`; these rows cover the version filter, including a
    //! randomised `SemVer` sweep, because the pattern must keep matching when a
    //! release adds a pre-release or build suffix.

    use super::{
        GENERATOR_VERSION_PATTERN, GENERATOR_VERSION_REPLACEMENT, redact_generator_version,
    };
    use anyhow::{Result, ensure};
    use proptest::prelude::*;
    use regex::Regex;
    use rstest::rstest;

    /// A document whose generator object carries `version`.
    fn diagnostic(version: &str) -> String {
        format!(
            "{{\n  \"generator\": {{\n    \"name\": \"netsuke\",\n    \
             \"version\": \"{version}\"\n  }},\n  \"diagnostics\": []\n}}"
        )
    }

    #[rstest]
    #[case("0.1.0", "a plain release version")]
    #[case("0.1.0-beta4", "a pre-release suffix changes nothing in the anchor")]
    fn generator_version_is_redacted(#[case] version: &str, #[case] because: &str) -> Result<()> {
        let actual = redact_generator_version(&diagnostic(version))?;

        ensure!(
            actual.contains("\"version\": \"[version]\""),
            "{because}: expected the placeholder, got: {actual}"
        );
        ensure!(
            !actual.contains(version),
            "{because}: the version text should not survive, got: {actual}"
        );
        Ok(())
    }

    #[test]
    fn another_object_version_stays_visible() -> Result<()> {
        let document = "{\n  \"tool\": {\n    \"name\": \"ninja\",\n    \
                        \"version\": \"1.11.1\"\n  },\n  \"generator\": {\n    \
                        \"name\": \"netsuke\",\n    \"version\": \"0.1.0\"\n  }\n}";

        let actual = redact_generator_version(document)?;

        ensure!(
            actual.contains("\"version\": \"1.11.1\""),
            "a version outside the generator object must stay visible, got: {actual}"
        );
        ensure!(
            actual.contains("\"version\": \"[version]\""),
            "the generator's version should still be redacted, got: {actual}"
        );
        Ok(())
    }

    #[test]
    fn a_version_quoted_in_a_message_stays_visible() -> Result<()> {
        // The message quotes the generator's own version, which is what a
        // value-based sweep of the whole document would also erase. Redaction
        // has to be positional: the generator object's field is incidental data
        // for a snapshot, while the same text inside an operator-facing message
        // is the diagnosis.
        let document = "{\n  \"generator\": {\n    \"name\": \"netsuke\",\n    \
                        \"version\": \"0.1.0\"\n  },\n  \"diagnostics\": [\n    \
                        {\"message\": \"netsuke 0.1.0 reported this\"}\n  ]\n}";

        let actual = redact_generator_version(document)?;

        ensure!(
            actual.contains("netsuke 0.1.0 reported this"),
            "a version the diagnostic quotes is operator-facing text, got: {actual}"
        );
        ensure!(
            actual.contains("\"version\": \"[version]\""),
            "the generator's own field should still be redacted, got: {actual}"
        );
        Ok(())
    }

    #[test]
    fn text_without_a_generator_object_is_unchanged() -> Result<()> {
        let text = "Validation failed for 'shells': value is \"0.1.0\"";

        let actual = redact_generator_version(text)?;

        ensure!(
            actual == text,
            "a text diagnostic carries no generator object, got: {actual}"
        );
        Ok(())
    }

    proptest! {
        /// The filter must keep matching whatever `SemVer` a release carries.
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
            let filter = match Regex::new(GENERATOR_VERSION_PATTERN) {
                Ok(filter) => filter,
                Err(error) => return Err(TestCaseError::fail(error.to_string())),
            };
            let filtered = filter.replace_all(&rendered, GENERATOR_VERSION_REPLACEMENT);

            prop_assert!(
                filtered.contains("\"version\": \"[version]\""),
                "the generator version should be redacted in {}",
                filtered,
            );
            prop_assert!(
                filtered.contains(&format!("\"version\": \"{unrelated_version}\"")),
                "a version outside the generator object should survive in {}",
                filtered,
            );
        }
    }
}
