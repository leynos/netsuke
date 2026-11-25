//! Property tests for the `case` tracker's security invariant.
//!
//! The fixed regressions beside this module pin the shapes each review round
//! found. This module states the invariant those shapes are instances of: a
//! marker written inside an open command substitution is never lowered for the
//! enclosing quote context, because the shell still reads it as substitution
//! source. The generated `case` grammar exercises that invariant across the
//! subjects, optional openers, patterns, and command prefixes the tracker
//! models.
//!
//! The property is deliberately an accept-or-refuse assertion rather than an
//! exact-output oracle. The scanner may over-approximate and refuse a recipe it
//! could have accepted; it may not accept one whose marker escapes the
//! substitution. Because that permits refusing everything, the sweep below also
//! counts preserved marker-free recipes, so a vacuous pass fails.

use proptest::prelude::*;

use super::super::*;

/// Subjects the header must read as data rather than as its `in` keyword.
const SUBJECTS: [&str; 5] = ["x", "in", "\"x\"", "x\"y\"in", "\"in\""];

/// Pattern lists paired with the optional opener Bash permits before them.
///
/// A bare `esac` is only a pattern when that opener precedes it: written
/// without one it terminates the construct, and the `)` after it is a syntax
/// error rather than a pattern terminator. Pairing the two keeps every entry
/// inside the shell grammar the invariant claims to cover.
const HEADERS: [(&str, &str); 9] = [
    ("", "x"),
    ("", "\"esac\""),
    ("", "x|esac"),
    ("", "in"),
    ("(", "x"),
    ("(", "esac"),
    ("(", "\"esac\""),
    ("(", "x|esac"),
    ("(", "in"),
];

/// Command lists for one arm, with [`MARKER`] standing in for a marker.
const ARMS: [&str; 6] = [
    "printf %s @MARK@",
    ": @MARK@",
    "coproc printf esac; printf %s @MARK@",
    "time printf %s @MARK@",
    "{ printf %s @MARK@; }",
    "printf %s \"x\"@MARK@",
];

/// Separators the tracker reads as ending an arm.
const SEPARATORS: [&str; 2] = [";;", ";;&"];

/// Placeholder the generators leave for the text under test.
const MARKER: &str = "@MARK@";

/// Spell a `case` construct from the chosen fragments.
fn case_body(subject: &str, header: (&str, &str), arm: &str, separator: &str) -> String {
    let (opener, pattern) = header;
    format!("case {subject} in {opener}{pattern}) {arm} {separator} esac")
}

/// Wrap a generated body in the double-quoted substitution under test.
///
/// The host's hostile binding encodes its path for the *outer* quote context,
/// so only a marker the shell reads inside the substitution can be lowered to
/// that encoding.
fn template(body: &str, replacement: &str) -> String {
    format!("echo \"$({body})\"").replace(MARKER, replacement)
}

/// Generate a `case` body with a marker inside its arm.
fn case_body_strategy() -> impl Strategy<Value = String> {
    (
        prop::sample::select(&SUBJECTS[..]),
        prop::sample::select(&HEADERS[..]),
        prop::sample::select(&ARMS[..]),
        prop::sample::select(&SEPARATORS[..]),
    )
        .prop_map(|(subject, header, arm, separator)| case_body(subject, header, arm, separator))
}

/// Build every marker-carrying template with its marker-free twin.
fn generated_templates() -> Vec<(String, String)> {
    let mut pairs = Vec::with_capacity(SUBJECTS.len() * HEADERS.len() * ARMS.len());
    for subject in SUBJECTS {
        for header in HEADERS {
            for arm in ARMS {
                for separator in SEPARATORS {
                    let body = case_body(subject, header, arm, separator);
                    pairs.push((template(&body, INS_TOKEN), template(&body, "y")));
                }
            }
        }
    }
    pairs
}

/// Check one template pair under both recipe shells.
///
/// Fails when any interpolator lowers the marker, since that lowering is the
/// defect the invariant forbids. On success, reports whether the marker-free
/// twin was preserved verbatim, which is the evidence that any refusal came
/// from the marker rather than from the surrounding shape.
fn check_pair(with_marker: &str, without_marker: &str) -> Result<bool, String> {
    let shells = [hostile_posix_bindings(), hostile_bash_bindings()];
    let mut is_preserved = false;
    for bindings in &shells {
        for interpolate in both_interpolators() {
            if let Ok(output) = interpolate(with_marker, bindings) {
                return Err(format!(
                    "a marker inside a substitution body must be refused, not lowered \
                     ({with_marker:?}): {output:?}"
                ));
            }
            match interpolate(without_marker, bindings) {
                Ok(output) if output == without_marker => is_preserved = true,
                Ok(output) => {
                    return Err(format!(
                        "a marker-free substitution must keep its text \
                         ({without_marker:?}): {output:?}"
                    ));
                }
                Err(_) => {}
            }
        }
    }
    Ok(is_preserved)
}

proptest! {
    /// Never lower a marker that the shell reads inside a `$()` body.
    ///
    /// Refusal is always acceptable here; the property fails only when a
    /// lowering succeeds while the marker sits inside the substitution.
    #[test]
    fn markers_inside_a_case_substitution_are_never_lowered_for_the_outer_context(
        body in case_body_strategy()
    ) {
        let with_marker = template(&body, INS_TOKEN);
        let without_marker = template(&body, "y");
        if let Err(reason) = check_pair(&with_marker, &without_marker) {
            return Err(TestCaseError::fail(reason));
        }
    }

    /// Keep the outcome when an unquoted continuation joins a keyword.
    ///
    /// Bash deletes the pair before parsing, so `c\<newline>ase` is the same
    /// keyword as `case`. `prop_assume` skips the pair where it would land
    /// inside a generated quote, because only the unquoted form splices.
    #[test]
    fn a_continued_keyword_does_not_change_the_outcome(
        head in "[ -~]{0,12}",
        tail in "[ -~]{0,12}",
    ) {
        prop_assume!(!head.contains(['"', '\\', '\'', '#', '<', '>']));
        prop_assume!(!tail.contains(['"', '\\', '\'', '#', '<', '>']));
        let literal = format!("echo \"$({head}c{tail}ase x in x) :;; esac)\"");
        let continued = format!("echo \"$({head}c\\\n{tail}ase x in x) :;; esac)\"");
        let bindings = bindings();
        for interpolate in both_interpolators() {
            let literal_outcome = interpolate(&literal, &bindings);
            let continued_outcome = interpolate(&continued, &bindings);
            prop_assert_eq!(
                literal_outcome.is_ok(),
                continued_outcome.is_ok(),
                "splicing a continuation must not change acceptance: {:?} vs {:?}",
                literal,
                continued
            );
        }
    }
}

/// Sweep the whole finite grammar rather than sampling it.
///
/// `proptest` may revisit the same entry of a finite table many times, so the
/// invariant is checked exhaustively here. The preserved count is the
/// liveliness guard: a tracker that refused every `case` construct would
/// satisfy the accept-or-refuse property vacuously, and this fails instead.
#[test]
fn every_generated_case_skeleton_refuses_a_marker_inside_the_substitution() {
    let pairs = generated_templates();
    let mut preserved = 0_usize;
    for (with_marker, without_marker) in &pairs {
        match check_pair(with_marker, without_marker) {
            Ok(is_preserved) => preserved += usize::from(is_preserved),
            Err(reason) => panic!("{reason}"),
        }
    }
    assert!(
        preserved > 0,
        concat!(
            "no marker-free skeleton was preserved, so the sweep proves ",
            "nothing about {} generated templates",
        ),
        pairs.len()
    );
}
