//! Tests for `PATHEXT` normalization.
//!
//! These call `parse_pathext` directly rather than capturing an `EnvSnapshot`,
//! so they mutate nothing and — more importantly — run on the Unix CI host.
//! The rules below were previously behind `#[cfg(windows)]` and so were never
//! executed by the suite that actually gates merges.

use super::env::{DEFAULT_PATHEXT, parse_pathext};
use rstest::rstest;
use std::ffi::OsStr;

fn parse(raw: &str) -> Vec<String> {
    parse_pathext(Some(OsStr::new(raw)))
}

/// The default list spelled out independently of the constant under test.
///
/// Written by hand so a change to `DEFAULT_PATHEXT` — a dropped entry, a
/// reordering — is caught here rather than silently agreeing with itself.
fn expected_default_pathext() -> Vec<String> {
    [
        ".com", ".exe", ".bat", ".cmd", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".wsh", ".msc",
    ]
    .iter()
    .copied()
    .map(String::from)
    .collect()
}

#[test]
fn unset_pathext_yields_the_default_list() {
    let expected = expected_default_pathext();
    assert_eq!(parse_pathext(None), expected);
    let constant: Vec<String> = DEFAULT_PATHEXT.iter().copied().map(String::from).collect();
    assert_eq!(constant, expected);
}

/// The default list itself must stay usable, not merely be echoed back.
///
/// Comparing the parse against `DEFAULT_PATHEXT` alone is tautological: were
/// the constant emptied or its entries mangled, that assertion would still
/// hold. These check the properties every consumer relies on.
#[test]
fn the_default_list_is_well_formed() {
    assert!(
        !DEFAULT_PATHEXT.is_empty(),
        "an empty default disables which"
    );
    for ext in DEFAULT_PATHEXT {
        assert!(ext.starts_with('.'), "{ext} should carry a leading dot");
        assert_eq!(*ext, ext.to_ascii_lowercase(), "{ext} should be lowercase");
    }
    for required in [".com", ".exe", ".bat", ".cmd"] {
        assert!(
            DEFAULT_PATHEXT.contains(&required),
            "the default list should include {required}"
        );
    }
}

/// An empty or whitespace-only value must not yield an empty extension list.
///
/// Windows would then treat nothing as executable, so `which` would report
/// every command missing. The fallback to the built-in list is what prevents a
/// blank `PATHEXT` from disabling resolution entirely.
#[rstest]
#[case::empty("")]
#[case::separators_only(";;;")]
#[case::whitespace_only("  ;  ; ")]
fn valueless_pathext_falls_back_to_the_default_list(#[case] raw: &str) {
    assert_eq!(parse(raw), DEFAULT_PATHEXT, "{raw:?} should fall back");
}

#[test]
fn extensions_are_lowercased() {
    assert_eq!(parse(".COM;.EXE"), vec![".com", ".exe"]);
}

#[test]
fn missing_leading_dots_are_inserted() {
    assert_eq!(parse("COM;EXE"), vec![".com", ".exe"]);
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    assert_eq!(parse(" .BAT ;\t.CMD\t"), vec![".bat", ".cmd"]);
}

/// De-duplication is case-insensitive and survives dot insertion, so `COM`,
/// `.com`, and `.COM` collapse to one entry.
#[test]
fn duplicates_collapse_after_normalization() {
    assert_eq!(parse("COM;.com;.COM; com "), vec![".com"]);
}

/// First occurrence wins, so author-declared precedence is preserved.
#[test]
fn declaration_order_is_preserved() {
    assert_eq!(parse(".exe;.bat;.com"), vec![".exe", ".bat", ".com"]);
}

/// A value contributing nothing usable behaves as if it were absent.
#[test]
fn entries_that_normalize_to_nothing_are_skipped() {
    assert_eq!(parse(".exe;;  ;.bat"), vec![".exe", ".bat"]);
}

mod properties {
    //! Property coverage for `parse_pathext`.
    //!
    //! The fixed cases above name specific behaviours; these state the
    //! invariants those cases are instances of, over inputs nobody would think
    //! to write down — stray whitespace, mixed case, repeated entries, empty
    //! segments, and combinations of all four.

    use super::{DEFAULT_PATHEXT, parse};
    use proptest::collection::vec;
    use proptest::prelude::*;

    /// One `PATHEXT` segment: optional whitespace, optional dot, mixed case.
    ///
    /// Deliberately includes segments that normalize to nothing, so the
    /// fallback path is generated rather than only reasoned about.
    fn segment() -> impl Strategy<Value = String> {
        // A deliberately small stem alphabet. With free-form stems, two
        // segments almost never collide, so `entries_are_unique` would pass
        // without ever seeing a duplicate — the generator, not the parser,
        // would be satisfying it.
        prop_oneof![
            3 => ("[ \t]*", prop::bool::ANY, "com|exe|bat|Com|EXE|Bat", "[ \t]*")
                .prop_map(|(lead, with_dot, stem, trail)| {
                    let dot = if with_dot { "." } else { "" };
                    format!("{lead}{dot}{stem}{trail}")
                }),
            1 => "[ \t]*".prop_map(|s: String| s),
        ]
    }

    fn raw_value() -> impl Strategy<Value = String> {
        vec(segment(), 0..8).prop_map(|parts| parts.join(";"))
    }

    proptest! {
        /// Every entry is lowercase, dot-prefixed, and non-empty after the dot.
        #[test]
        fn entries_are_normalized(raw in raw_value()) {
            for ext in parse(&raw) {
                prop_assert!(ext.starts_with('.'), "missing dot: {ext:?}");
                prop_assert!(ext.len() > 1, "nothing after the dot: {ext:?}");
                prop_assert_eq!(&ext, &ext.to_ascii_lowercase());
            }
        }

        /// No two entries name the same extension, compared case-insensitively.
        ///
        /// Deliberately not "no exact duplicates": the parser collects into an
        /// `IndexSet`, so exact uniqueness holds structurally whatever else it
        /// does, and asserting it would pass even with normalization removed.
        /// Comparing case-insensitively is what makes this a claim about
        /// `parse_pathext` rather than about `IndexSet`.
        #[test]
        fn entries_are_unique(raw in raw_value()) {
            let parsed = parse(&raw);
            let mut seen = std::collections::HashSet::new();
            for ext in &parsed {
                prop_assert!(
                    seen.insert(ext.to_ascii_lowercase()),
                    "{ext:?} repeats an earlier entry in {parsed:?} bar case"
                );
            }
        }

        /// Re-parsing an already-parsed list changes nothing.
        ///
        /// Idempotence is what lets a caller pass a normalized value back in —
        /// and it fails immediately if normalization is order-dependent or if
        /// the dot prefix is applied twice.
        #[test]
        fn parsing_is_idempotent(raw in raw_value()) {
            let once = parse(&raw);
            let twice = parse(&once.join(";"));
            prop_assert_eq!(once, twice);
        }

        /// An input with no usable segment yields the built-in list.
        ///
        /// Generated rather than enumerated: any join of whitespace-only
        /// segments must fall back, however many there are.
        #[test]
        fn unusable_input_falls_back(parts in vec("[ \t]*", 0..6)) {
            prop_assert_eq!(parse(&parts.join(";")), DEFAULT_PATHEXT.to_vec());
        }

        /// The first normalized occurrence fixes an entry's position.
        ///
        /// Order is not cosmetic: it is the order `which` tries extensions in,
        /// so a later duplicate must not move an earlier entry.
        #[test]
        fn first_occurrence_fixes_order(stems in vec("[a-z][a-z0-9]{0,3}", 1..5)) {
            let mut expected: Vec<String> = Vec::new();
            for stem in &stems {
                let ext = format!(".{stem}");
                if !expected.contains(&ext) {
                    expected.push(ext);
                }
            }
            // Append an upper-case repeat of every stem: each is a duplicate
            // under normalization, so none may appear again or displace another.
            let raw = stems
                .iter()
                .map(|s| format!(".{s}"))
                .chain(stems.iter().map(|s| s.to_ascii_uppercase()))
                .collect::<Vec<_>>()
                .join(";");
            prop_assert_eq!(parse(&raw), expected);
        }
    }
}
