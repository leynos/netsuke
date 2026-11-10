//! Shared fixtures for the POSIX lexical scanner's unit tests.
//!
//! The heredoc and comment-boundary suites drive the same two traversals over
//! the same hostile bindings. Keeping that scaffolding here leaves each suite
//! free to describe only the case it pins.

use super::super::super::{
    CommandBindings, INS_TOKEN, IrGenError, OUTS_TOKEN, RecipeShell,
    interpolate_command_with_bindings, interpolate_script_with_bindings,
};
use super::Interpolator;

use camino::Utf8PathBuf;

/// Path whose unquoted encoding is executable shell syntax inside double quotes.
pub(super) const HOSTILE_PATH: &str = "x\"; touch injected; echo \"y";

/// Expect the hostile path escaped for insertion between existing double quotes.
pub(super) const HOSTILE_DOUBLE_QUOTED: &str = "x\\\"; touch injected; echo \\\"y";

/// Both POSIX interpolators that share the comment scanner.
const BOTH_INTERPOLATORS: [Interpolator; 2] = [
    interpolate_command_with_bindings,
    interpolate_script_with_bindings,
];

/// The script interpolator on its own.
///
/// A `command:` recipe cannot express the script-only forms, so a case that
/// needs a heredoc or a second line exercises the script traversal alone.
const SCRIPT_INTERPOLATOR: [Interpolator; 1] = [interpolate_script_with_bindings];

/// Build bindings for one simple input and output path pair.
pub(super) fn bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    )
}

/// Build POSIX bindings that expose a wrong quote-context encoding.
pub(super) fn hostile_posix_bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(HOSTILE_PATH)],
        &[Utf8PathBuf::from(HOSTILE_PATH)],
        RecipeShell::Posix,
    )
}

/// Return both POSIX interpolators that share the comment scanner.
///
/// A boundary case that holds for one traversal should hold for the other.
pub(super) fn both_interpolators() -> [Interpolator; 2] {
    BOTH_INTERPOLATORS
}

/// Select which POSIX interpolators an assertion exercises.
#[derive(Clone, Copy)]
pub(super) enum Interpolators {
    /// Both the command and the script traversal.
    Both,
    /// Only the script traversal, which `command:` recipes cannot reach.
    ScriptOnly,
}

impl Interpolators {
    /// Return the interpolator functions this selection exercises.
    const fn select(self) -> &'static [Interpolator] {
        match self {
            Self::Both => &BOTH_INTERPOLATORS,
            Self::ScriptOnly => &SCRIPT_INTERPOLATOR,
        }
    }
}

/// Assert that the selected interpolators yield the expected text.
pub(super) fn assert_output(
    template: &str,
    expected: &str,
    context: &str,
    interpolators: Interpolators,
) {
    let bindings = hostile_posix_bindings();
    for interpolate in interpolators.select() {
        assert_eq!(
            interpolate(template, &bindings).expect(context),
            expected,
            "{context}"
        );
    }
}

/// Assert that command and script interpolation both reject the template.
pub(super) fn assert_both_reject(template: &str, context: &str) {
    let bindings = hostile_posix_bindings();
    for interpolate in both_interpolators() {
        assert!(interpolate(template, &bindings).is_err(), "{context}");
    }
}

/// Assert that both interpolators report an ambiguous substitution boundary.
///
/// The over-approximating guards must fail closed through the dedicated
/// diagnostic rather than through a generic invalid-command error, so a caller
/// can still tell that the scanner declined to classify the boundary.
pub(super) fn assert_both_reject_as_ambiguous(template: &str, context: &str) {
    let bindings = hostile_posix_bindings();
    for interpolate in both_interpolators() {
        let error = interpolate(template, &bindings).expect_err(context);
        assert!(
            matches!(error, IrGenError::AmbiguousSubstitutionBoundary { .. }),
            "{context}: unexpected error {error:?}"
        );
    }
}

/// Build a pair whose comment carries a marker and an unmatched quote.
///
/// A scanner that ends the comment early treats either as executable syntax and
/// mis-encodes the double-quoted marker on the following line. `closer` is an
/// extra line between the comment and that marker line, used when a
/// substitution must be closed before interpolation continues.
pub(super) fn inert_comment_case(prefix: &str, closer: Option<&str>) -> (String, String) {
    let continuation = closer.map_or_else(String::new, |line| format!("\n{line}"));
    let comment = format!("{prefix}# {INS_TOKEN} unmatched '{continuation}");
    (
        format!("{comment}\necho \"{OUTS_TOKEN}\""),
        format!("{comment}\necho \"{HOSTILE_DOUBLE_QUOTED}\""),
    )
}

/// Build a pair where `#` must stay in the word so its quote stays executable.
pub(super) fn executable_quote_case(prefix: &str) -> (String, String) {
    (
        format!("{prefix}#\"\necho {INS_TOKEN}\""),
        format!("{prefix}#\"\necho {HOSTILE_DOUBLE_QUOTED}\""),
    )
}
