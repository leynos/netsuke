//! Property tests for literal-prefix extraction and capability relativisation.
//!
//! The fixed cases elsewhere in this module pin a handful of shapes. These
//! cover the invariants those shapes are examples of, across arbitrary
//! metacharacter placement: that the extracted prefix really is a
//! metacharacter-free directory prefix and really is the longest one, and that
//! relativising a match against it accepts exactly the paths inside the prefix
//! and rejects everything else.
//!
//! Both properties are pure. The [`GlobRoot`] used for relativisation holds a
//! capability that its prefix logic never consults, so one handle on the
//! working directory serves every case.

use super::super::walk::{GlobRoot, literal_dir_prefix};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use proptest::prelude::*;
use std::path::MAIN_SEPARATOR;

const METACHARACTERS: [char; 4] = ['*', '?', '[', '{'];

/// Generate patterns mixing literal segments, separators and metacharacters.
fn pattern() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            "[a-z]{1,4}".prop_map(|s| s),
            Just(MAIN_SEPARATOR.to_string()),
            Just("*".to_owned()),
            Just("?".to_owned()),
            Just("[ab]".to_owned()),
            Just("{a,b}".to_owned()),
        ],
        0..8,
    )
    .prop_map(|parts| parts.concat())
}

/// Build a `GlobRoot` at `prefix` whose capability is never dereferenced.
fn root_at(prefix: &str) -> Result<GlobRoot, TestCaseError> {
    let dir = Dir::open_ambient_dir(".", ambient_authority())
        .map_err(|err| TestCaseError::fail(format!("open the working directory: {err}")))?;
    Ok(GlobRoot::new(dir, Utf8PathBuf::from(prefix)))
}

proptest! {
    /// The prefix is `.` or a genuine prefix of the pattern.
    #[test]
    fn prefix_is_a_prefix_of_the_pattern(pattern in pattern()) {
        let prefix = literal_dir_prefix(&pattern);
        prop_assert!(
            prefix == "." || pattern.starts_with(prefix),
            "prefix {prefix:?} is not a prefix of {pattern:?}"
        );
    }

    /// The prefix never contains a glob metacharacter.
    #[test]
    fn prefix_is_free_of_metacharacters(pattern in pattern()) {
        let prefix = literal_dir_prefix(&pattern);
        prop_assert!(
            !prefix.contains(METACHARACTERS),
            "prefix {prefix:?} of {pattern:?} contains a metacharacter"
        );
    }

    /// The prefix is `.` or a directory path ending at a separator.
    #[test]
    fn prefix_is_a_directory_path(pattern in pattern()) {
        let prefix = literal_dir_prefix(&pattern);
        prop_assert!(
            prefix == "." || prefix.ends_with(MAIN_SEPARATOR),
            "prefix {prefix:?} of {pattern:?} is not a directory path"
        );
    }

    /// The prefix is the longest one available: what follows it holds no
    /// further separator that is still free of metacharacters.
    #[test]
    fn prefix_is_maximal(pattern in pattern()) {
        let prefix = literal_dir_prefix(&pattern);
        let consumed = if prefix == "." { 0 } else { prefix.len() };
        let Some(rest) = pattern.get(consumed..) else {
            return Err(TestCaseError::fail("prefix is not a character boundary"));
        };
        let literal_rest = rest
            .find(METACHARACTERS)
            .map_or(Some(rest), |idx| rest.get(..idx))
            .ok_or_else(|| TestCaseError::fail("metacharacter is not a character boundary"))?;
        prop_assert!(
            !literal_rest.contains(MAIN_SEPARATOR),
            "prefix {prefix:?} of {pattern:?} left a literal separator in {literal_rest:?}"
        );
    }

    /// Any path under the prefix relativises to its remainder.
    #[test]
    fn matches_inside_the_prefix_relativise(
        prefix in "[a-z]{1,4}(/[a-z]{1,4}){0,3}",
        tail in "[a-z]{1,4}(/[a-z]{1,4}){0,3}",
    ) {
        let root = root_at(&prefix)?;
        let matched = Utf8PathBuf::from(&prefix).join(&tail);
        let relative = root
            .relativise(&matched)
            .map_err(|err| TestCaseError::fail(format!("{matched} should relativise: {err}")))?;
        prop_assert_eq!(relative, Utf8Path::new(&tail));
    }

    /// The prefix itself relativises to the capability root.
    #[test]
    fn the_prefix_itself_relativises_to_the_root(prefix in "[a-z]{1,4}(/[a-z]{1,4}){0,3}") {
        let root = root_at(&prefix)?;
        let matched = Utf8PathBuf::from(&prefix);
        let relative = root
            .relativise(&matched)
            .map_err(|err| TestCaseError::fail(format!("{matched} should relativise: {err}")))?;
        prop_assert_eq!(relative, Utf8Path::new("."));
    }

    /// Any path outside the prefix is rejected rather than resolved through a
    /// wider capability.
    #[test]
    fn matches_outside_the_prefix_are_rejected(
        prefix in "[a-z]{1,4}(/[a-z]{1,4}){0,3}",
        outside in "[a-z]{1,4}(/[a-z]{1,4}){0,3}",
    ) {
        prop_assume!(!Utf8Path::new(&outside).starts_with(&prefix));
        let root = root_at(&prefix)?;
        let err = root
            .relativise(Utf8Path::new(&outside))
            .err()
            .ok_or_else(|| TestCaseError::fail(format!("{outside} escaped prefix {prefix}")))?;
        prop_assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
