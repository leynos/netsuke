//! Tests for the home-directory precedence ladders.
//!
//! The ladders are deliberately tested at this seam rather than through the
//! `expanduser` `MiniJinja` filter. Driving the filter with real environment
//! combinations would require mutating the process environment in-process,
//! which AGENTS.md forbids; the seam exists precisely so the precedence
//! logic is reachable without that (#486). The filter's own concerns — `~`
//! recognition, the named-user rejection, and the no-home error — are
//! exercised below on `expanduser` itself through the injected
//! [`HomeDirectory`] value the filter registration receives; the
//! filter-level suite in `tests/std_filter_tests` is dark pending #520.
//!
//! Both ladders are driven through their `read_env` closure, so nothing here
//! mutates the process environment and the cases run concurrently. Neither
//! helper carries a `cfg` gate: they hold no platform-specific selection logic
//! themselves — `home_from_env` picks between them — so both compile and are
//! exercised on every host. That matters most for the Windows ladder, the more
//! intricate of the two, which would otherwise be unreachable from the Unix CI
//! host.

use super::path_utils::{HomeSource, posix_home_from, windows_home_from};
use crate::stdlib::config_types::HomeDirectory;
use rstest::rstest;

/// Borrow a resolution as `(home, source)` so cases can be written as literals.
fn as_pair(resolved: Option<&HomeSource>) -> Option<(&str, &str)> {
    resolved.map(|(home, source)| (home.as_str(), *source))
}

mod expanduser_behaviour {
    //! `expanduser` resolves the home through the injected [`HomeDirectory`]
    //! value and `read_env` reader the filter registration supplies, so its
    //! behaviour is testable here without touching the process environment.
    //! Every branch below is drivable that way: `Explicit` supplies a home,
    //! `Missing` models a host without one, and `Ambient` walks the platform
    //! ladder through the injected reader — the isolation the seam exists to
    //! provide. Cases whose branch never consults the environment pass
    //! `|_| None` to prove they do not.

    use super::super::path_utils::expanduser;
    use super::HomeDirectory;
    use crate::test_tracing_capture::with_test_subscriber;
    use rstest::rstest;
    use tracing_subscriber::filter::LevelFilter;

    #[rstest]
    #[case::tilde_alone("~", "/home/a")]
    #[case::tilde_with_path("~/notes", "/home/a/notes")]
    fn expands_against_the_resolved_home(#[case] raw: &str, #[case] expected: &str) {
        let home = HomeDirectory::Explicit("/home/a".to_owned());
        let expanded = expanduser(raw, &home, |_| None).expect("expansion should succeed");
        assert_eq!(expanded, expected);
    }

    /// `Ambient` resolves through the injected reader, not process state:
    /// a reader supplying HOME drives the expansion without any process
    /// environment involvement.
    #[test]
    fn ambient_resolves_through_the_injected_reader() {
        let read_env = |key: &str| (key == "HOME").then(|| "/injected/home".to_owned());
        let expanded = expanduser("~/x", &HomeDirectory::Ambient, read_env)
            .expect("the injected reader should supply the home");
        assert_eq!(expanded, "/injected/home/x");
    }

    /// `Ambient` with a reader that finds nothing is the no-home error.
    #[test]
    fn ambient_with_an_empty_reader_is_an_error() {
        let error = expanduser("~/x", &HomeDirectory::Ambient, |_| None)
            .expect_err("an empty reader should leave no home");
        assert_eq!(error.kind(), minijinja::ErrorKind::InvalidOperation);
    }

    /// Capture the home-resolution events emitted by one `expanduser` call.
    fn events_for(
        raw: &str,
        home: &HomeDirectory,
        read_env: impl Fn(&str) -> Option<String>,
    ) -> Vec<String> {
        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            drop(expanduser(raw, home, read_env));
            captured.snapshot()
        })
        .into_iter()
        .filter(|event| event.contains("stdlib.expanduser.home"))
        .collect()
    }

    /// A successful ambient resolution reports the rung that supplied the home,
    /// and nothing else: no path, no environment value.
    #[test]
    fn resolution_reports_its_bounded_source() {
        let read_env = |key: &str| (key == "USERPROFILE").then(|| "/injected/home".to_owned());
        let events = events_for("~/x", &HomeDirectory::Ambient, read_env);
        let event = events.first().expect("one home-resolution event");
        assert!(
            event.contains("source=\"userprofile\"") && event.contains("found=true"),
            "the event should name the rung and the outcome: {event}"
        );
        assert!(
            !events.iter().any(|other| other.contains("/injected/home")),
            "no event may carry the resolved home: {events:?}"
        );
    }

    /// A resolution that finds nothing records the bounded failure category
    /// alongside the `missing` source.
    #[test]
    fn an_unresolvable_home_reports_a_bounded_outcome() {
        let events = events_for("~/x", &HomeDirectory::Missing, |_| None);
        assert!(
            events.iter().any(|event| {
                event.contains("outcome=\"home_unavailable\"")
                    && event.contains("source=\"missing\"")
            }),
            "the failure event should carry the bounded outcome: {events:?}"
        );
    }

    /// A path without a leading `~` passes through unchanged, and the home
    /// source is never consulted: `Missing` would otherwise make this an
    /// error.
    #[test]
    fn a_non_tilde_path_passes_through() {
        let expanded = expanduser("/etc/hosts", &HomeDirectory::Missing, |_| None)
            .expect("a non-tilde path should pass through");
        assert_eq!(expanded, "/etc/hosts");
    }

    /// Named-user expansion is rejected before the home source is consulted.
    #[test]
    fn named_user_forms_are_rejected() {
        let error = expanduser("~alice/notes", &HomeDirectory::Missing, |_| None)
            .expect_err("~alice should be rejected");
        assert_eq!(error.kind(), minijinja::ErrorKind::InvalidOperation);
    }

    /// A tilde with no resolvable home is an error, not a passthrough.
    #[test]
    fn a_missing_home_is_an_error() {
        let error = expanduser("~/notes", &HomeDirectory::Missing, |_| None)
            .expect_err("no home should be an error");
        assert_eq!(error.kind(), minijinja::ErrorKind::InvalidOperation);
    }
}

/// Build a lookup over a fixed set of pairs; anything absent reads as unset.
fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// Each case pins the resolved home *and* the bounded label naming the rung
/// that supplied it, since the label is what home-resolution telemetry reports.
#[rstest]
#[case::home_wins(&[("HOME", "/home/a"), ("USERPROFILE", "/users/b")], Some(("/home/a", "home")))]
#[case::falls_back_to_userprofile(&[("USERPROFILE", "/users/b")], Some(("/users/b", "userprofile")))]
#[case::nothing_set(&[], None)]
// An empty value is passed through rather than treated as unset: the ladder
// reports what the environment says, and `expanduser` decides what an empty
// home means. Pinned so the behaviour cannot drift silently.
#[case::empty_home_is_passed_through(&[("HOME", "")], Some(("", "home")))]
#[case::empty_userprofile_is_passed_through(&[("USERPROFILE", "")], Some(("", "userprofile")))]
fn posix_ladder(#[case] pairs: &[(&str, &str)], #[case] expected: Option<(&str, &str)>) {
    let resolved = posix_home_from(env_of(pairs));
    assert_eq!(as_pair(resolved.as_ref()), expected);
}

/// As above: every Windows rung is pinned by both value and source label.
#[rstest]
#[case::home_wins(
    &[("HOME", "H:\\home"), ("USERPROFILE", "U:\\users"), ("HOMEDRIVE", "C:"), ("HOMEPATH", "\\me")],
    Some(("H:\\home", "home"))
)]
#[case::userprofile_before_the_pair(
    &[("USERPROFILE", "U:\\users"), ("HOMEDRIVE", "C:"), ("HOMEPATH", "\\me")],
    Some(("U:\\users", "userprofile"))
)]
#[case::drive_and_path_pair(
    &[("HOMEDRIVE", "C:"), ("HOMEPATH", "\\me")],
    Some(("C:\\me", "drive_path"))
)]
#[case::homeshare_last(
    &[("HOMESHARE", "\\\\server\\share")],
    Some(("\\\\server\\share", "homeshare"))
)]
#[case::nothing_set(&[], None)]
fn windows_ladder(#[case] pairs: &[(&str, &str)], #[case] expected: Option<(&str, &str)>) {
    let resolved = windows_home_from(env_of(pairs));
    assert_eq!(as_pair(resolved.as_ref()), expected);
}

/// An incomplete `HOMEDRIVE`/`HOMEPATH` pair must not be combined.
///
/// Joining them would yield a bare `C:`, which names a drive rather than a home
/// directory and would silently expand `~` to the drive root. The ladder falls
/// through to `HOMESHARE` instead.
#[rstest]
#[case::falls_through_to_homeshare(
    &[("HOMEDRIVE", "C:"), ("HOMEPATH", ""), ("HOMESHARE", "\\\\server\\share")],
    Some(("\\\\server\\share", "homeshare"))
)]
#[case::no_homeshare_yields_none(&[("HOMEDRIVE", "C:"), ("HOMEPATH", "")], None)]
// An empty HOMEDRIVE is equally incomplete: combining it would yield a bare
// `\me`, naming a path on the current drive rather than a home directory.
#[case::empty_homedrive_falls_through_to_homeshare(
    &[("HOMEDRIVE", ""), ("HOMEPATH", "\\me"), ("HOMESHARE", "\\\\server\\share")],
    Some(("\\\\server\\share", "homeshare"))
)]
#[case::empty_homedrive_without_homeshare_yields_none(
    &[("HOMEDRIVE", ""), ("HOMEPATH", "\\me")],
    None
)]
#[case::both_halves_empty_yields_none(&[("HOMEDRIVE", ""), ("HOMEPATH", "")], None)]
fn windows_incomplete_drive_pair_is_treated_as_unset(
    #[case] pairs: &[(&str, &str)],
    #[case] expected: Option<(&str, &str)>,
) {
    let resolved = windows_home_from(env_of(pairs));
    assert_eq!(as_pair(resolved.as_ref()), expected);
}

/// A `HOMEDRIVE` without `HOMEPATH` is incomplete and must not be used alone.
#[test]
fn windows_homedrive_without_homepath_falls_through() {
    let pairs = [("HOMEDRIVE", "C:")];
    assert_eq!(windows_home_from(env_of(&pairs)), None);
}

mod properties {
    //! Property coverage for the precedence ladders.
    //!
    //! The fixed cases above pin named rungs; these state the whole contract
    //! at once over every combination of set, unset, and empty variables, so
    //! a drift in one rung — a dropped emptiness check, a reordered
    //! fallback — fails here even if no enumerated case names it.

    use super::{posix_home_from, windows_home_from};
    use proptest::option;
    use proptest::prelude::*;

    /// A variable's value; deliberately often empty, so the incomplete-pair
    /// rule is generated rather than only enumerated.
    fn value() -> impl Strategy<Value = String> {
        prop_oneof![
            1 => Just(String::new()),
            3 => "[a-z]{1,3}".prop_map(|s| s),
        ]
    }

    /// Lookup over the five optional variables.
    fn reader<'a>(
        pairs: &'a [(&'a str, &'a Option<String>)],
    ) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .and_then(|(_, value)| (*value).clone())
        }
    }

    proptest! {
        /// The Windows ladder matches its documented precedence exactly.
        ///
        /// The expectation is computed from the documented contract — `HOME`,
        /// then `USERPROFILE`, then a non-empty `HOMEDRIVE`/`HOMEPATH` pair,
        /// then `HOMESHARE` — not by calling the ladder, so an implementation
        /// change that departs from the contract cannot agree with itself.
        #[test]
        fn windows_ladder_matches_its_documented_precedence(
            home in option::of(value()),
            userprofile in option::of(value()),
            homedrive in option::of(value()),
            homepath in option::of(value()),
            homeshare in option::of(value()),
        ) {
            let expected = home.clone().map(|value| (value, "home"))
                .or_else(|| userprofile.clone().map(|value| (value, "userprofile")))
                .or_else(|| {
                    match (&homedrive, &homepath) {
                        (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
                            Some((format!("{drive}{path}"), "drive_path"))
                        }
                        _ => homeshare.clone().map(|value| (value, "homeshare")),
                    }
                });
            let pairs = [
                ("HOME", &home),
                ("USERPROFILE", &userprofile),
                ("HOMEDRIVE", &homedrive),
                ("HOMEPATH", &homepath),
                ("HOMESHARE", &homeshare),
            ];
            prop_assert_eq!(windows_home_from(reader(&pairs)), expected);
        }

        /// The POSIX ladder reads only `HOME` and `USERPROFILE`.
        ///
        /// Its result must equal the two-variable precedence and must not
        /// change when the Windows-only variables are present, whatever their
        /// values.
        #[test]
        fn posix_ladder_reads_only_home_and_userprofile(
            home in option::of(value()),
            userprofile in option::of(value()),
            homedrive in option::of(value()),
            homepath in option::of(value()),
            homeshare in option::of(value()),
        ) {
            let expected = home.clone().map(|value| (value, "home"))
                .or_else(|| userprofile.clone().map(|value| (value, "userprofile")));
            let all = [
                ("HOME", &home),
                ("USERPROFILE", &userprofile),
                ("HOMEDRIVE", &homedrive),
                ("HOMEPATH", &homepath),
                ("HOMESHARE", &homeshare),
            ];
            prop_assert_eq!(posix_home_from(reader(&all)), expected);
        }
    }
}
