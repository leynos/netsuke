//! The temporary-path filter shared by both snapshot trees.
//!
//! Insta settings live in two crates — `src/snapshot_test_support.rs` serves
//! `src/snapshots/`, and `tests/manifest_load_options/fixtures/settings.rs`
//! serves `tests/snapshots/` — and neither can reach the other, because the
//! first is `#[cfg(test)]` code inside the library and the second is a separate
//! integration crate. The pure pattern builder is therefore shared from here,
//! while each crate wires it into its own settings object.
//!
//! Sharing the builder rather than the settings is deliberate. The two settings
//! objects differ beyond the filter — snapshot directory and module-prefix
//! handling are each crate's own business — and the filter is the only part
//! that cannot be written against one crate's snapshot text alone. It is also
//! the only part whose defect is invisible on the recording platform, which is
//! exactly the property that made it worth centralizing: two copies can drift
//! apart and still pass, because each is guarded only by its own copy of the
//! same cases.
//!
//! # Examples
//!
//! ```rust
//! use test_support::temp_path_filter::temp_dir_filter_pattern;
//!
//! // Each separator becomes a class matching one *or more* of either kind,
//! // which is what lets one pattern cover the JSON-escaped spelling too.
//! let pattern = temp_dir_filter_pattern(r"C:\Temp");
//! assert!(pattern.contains(r"[\\/]+"));
//! ```

/// Build the pattern that replaces a temporary directory with `[TMP]`.
///
/// Each separator becomes a class matching one or more of either separator, and
/// every other character is escaped. The class is what lets one pattern cover
/// both spellings a snapshot can hold: a diagnostic quotes a Windows path with
/// the single backslashes the platform renders, while the serialized half of the
/// same snapshot doubles them, so a pattern requiring exactly one separator
/// would leave the serialized copy unnormalized — and it is the serialized copy
/// that carries the run-specific workspace name.
///
/// The `+` is load-bearing. A first repair widened a literal `/` to the class
/// `[\\/]`, which reads as "either separator" but still matches exactly one
/// character, so the doubled form stayed unmatched.
///
/// A trailing separator is dropped first, because leaving it in would make two
/// adjacent classes demand two separators where the text has one.
///
/// Requiring a whole component at each step, rather than matching the directory
/// name as a bare substring, is what leaves a directory that merely starts with
/// the same characters alone: `/tmp` would otherwise rewrite an unrelated
/// `/tmpfoo/` component in a path or in a diagnostic.
///
/// The placeholder is the same on every platform, which matters because the
/// snapshots are checked in: a row that records only `[TMP]` reads the same
/// whichever machine recorded it.
///
/// The directory arrives as an argument rather than being read from
/// `std::env::temp_dir()` here. That keeps the function pure, which is what
/// lets a test assert the Windows spelling on any host — the defect this
/// guards is invisible on Linux, so a test reading the ambient directory could
/// only ever check whatever platform happened to record the snapshot.
#[must_use]
pub fn temp_dir_filter_pattern(temp_dir: &str) -> String {
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
    use anyhow::Result;
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
        anyhow::ensure!(
            actual == expected,
            "{because}: expected {expected:?}, got {actual:?}"
        );
        Ok(())
    }
}
