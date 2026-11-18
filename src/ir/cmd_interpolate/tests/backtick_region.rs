//! Script-mode regressions for backtick quote-state ownership (#1000).
//!
//! A backtick region saves the quote context of the shell region that opened
//! it. These cases pin that the closing backtick restores that region rather
//! than the script-level field, which would make a later marker use the wrong
//! path encoding. They also pin that nesting which closes a region before its
//! backtick fails closed instead of corrupting the enclosing quote state.

use super::*;
use rstest::rstest;

/// Build bindings whose encoded text names the quote context that was chosen.
///
/// Other cases exercise the production path encoder. These sentinels let a
/// case read the selected context straight out of the lowered script, so a
/// corrupted context is visible as a wrong encoding rather than an accident of
/// a path that happens to quote the same way in two contexts.
fn context_bindings(shell: RecipeShell) -> CommandBindings {
    CommandBindings {
        shell,
        ins: PathSubstitutions {
            single_quoted: "SGL".to_owned(),
            double_quoted: "DBL".to_owned(),
            unquoted: "UNQ".to_owned(),
        },
        outs: PathSubstitutions {
            single_quoted: "OSGL".to_owned(),
            double_quoted: "ODBL".to_owned(),
            unquoted: "OUNQ".to_owned(),
        },
    }
}

/// Name the quoting of the enclosing string and of the backtick body.
///
/// The two regions vary independently, and the defect under test needs the
/// backtick body quoted differently from the region around it.
#[derive(Clone, Copy)]
enum QuoteLayout {
    /// Leave the enclosing string and the backtick body unquoted.
    NoQuotes,
    /// Leave the enclosing string unquoted and double-quote the backtick body.
    BodyQuoted,
    /// Double-quote the enclosing string and leave the backtick body unquoted.
    OuterQuoted,
    /// Double-quote the enclosing string and the backtick body.
    BothQuoted,
}

impl QuoteLayout {
    /// Render the backtick body this layout describes.
    const fn body(self) -> &'static str {
        match self {
            Self::BodyQuoted | Self::BothQuoted => "\"`printf x`\"",
            Self::NoQuotes | Self::OuterQuoted => "`printf x`",
        }
    }

    /// Report whether this layout double-quotes the enclosing string.
    const fn is_outer_quoted(self) -> bool {
        matches!(self, Self::OuterQuoted | Self::BothQuoted)
    }
}

/// Restore a backtick's saved context to the `$()` region that opened it.
///
/// The enclosing substitution opens at script level, so the marker after it is
/// outside quotes when the outer string is unquoted and inside double quotes
/// when the whole substitution sits in one. Either way the marker's encoding
/// comes from the region that owns it, not from the innermost frame the
/// backtick happened to open in.
#[rstest]
#[case::ins_unquoted_outer_unquoted_body(
    RecipeShell::Posix,
    INS_TOKEN,
    QuoteLayout::NoQuotes,
    "UNQ"
)]
#[case::ins_unquoted_outer_quoted_body(
    RecipeShell::Posix,
    INS_TOKEN,
    QuoteLayout::BodyQuoted,
    "UNQ"
)]
#[case::ins_quoted_outer_unquoted_body(
    RecipeShell::Posix,
    INS_TOKEN,
    QuoteLayout::OuterQuoted,
    "DBL"
)]
#[case::ins_quoted_outer_quoted_body(RecipeShell::Posix, INS_TOKEN, QuoteLayout::BothQuoted, "DBL")]
#[case::outs_unquoted_outer_quoted_body(
    RecipeShell::Posix,
    OUTS_TOKEN,
    QuoteLayout::BodyQuoted,
    "OUNQ"
)]
#[case::outs_quoted_outer_unquoted_body(
    RecipeShell::Posix,
    OUTS_TOKEN,
    QuoteLayout::OuterQuoted,
    "ODBL"
)]
#[case::bash_unquoted_outer_quoted_body(
    RecipeShell::Bash,
    INS_TOKEN,
    QuoteLayout::BodyQuoted,
    "UNQ"
)]
#[case::bash_quoted_outer_quoted_body(
    RecipeShell::Bash,
    OUTS_TOKEN,
    QuoteLayout::BothQuoted,
    "ODBL"
)]
fn script_marker_after_backtick_in_command_substitution_uses_owning_region(
    #[case] shell: RecipeShell,
    #[case] marker: &str,
    #[case] layout: QuoteLayout,
    #[case] expected: &str,
) {
    let substitution = format!("$(printf %s {})", layout.body());
    let template = if layout.is_outer_quoted() {
        format!("printf '%s' \"{substitution} {marker}\"")
    } else {
        format!("printf '%s' {substitution} {marker}")
    };

    let actual = interpolate_script_with_bindings(&template, &context_bindings(shell))
        .expect("a backtick that closes in its owning $() must lower");
    assert_eq!(
        actual,
        template.replace(marker, expected),
        "a marker after {template:?} must use its own region's encoding"
    );
}

/// Restore a top-level backtick's context into its surrounding quote region.
#[rstest]
#[case::unquoted("printf '%s' `printf x` {marker}", "UNQ")]
#[case::after_double_quoted("printf '%s' \"`printf x`\" {marker}", "UNQ")]
#[case::inside_double_quotes("printf '%s' \"`printf x` {marker}\"", "DBL")]
fn script_marker_after_top_level_backtick_uses_its_surrounding_quotes(
    #[case] template: &str,
    #[case] expected: &str,
) {
    let command = template.replace("{marker}", INS_TOKEN);
    let actual = interpolate_script_with_bindings(&command, &context_bindings(RecipeShell::Posix))
        .expect("a top-level backtick region must lower");
    assert_eq!(
        actual,
        command.replace(INS_TOKEN, expected),
        "a marker after {command:?} must use its surrounding quotes"
    );
}

/// Keep a `$()` opened inside a backtick valid while it closes first.
#[test]
fn script_command_substitution_inside_backtick_restores_script_context() {
    let bindings = context_bindings(RecipeShell::Posix);
    let template = format!("printf '%s' `echo $(date)` {INS_TOKEN}");
    let actual = interpolate_script_with_bindings(&template, &bindings)
        .expect("a $() that closes before its backtick must remain supported");
    assert_eq!(
        actual, "printf '%s' `echo $(date)` UNQ",
        "a nested $() that closes before its backtick must leave script level unquoted"
    );
}

/// Restore the innermost region's context after two levels of `$()` nesting.
#[test]
fn script_two_level_command_substitution_inside_backtick_restores_outer_context() {
    let bindings = context_bindings(RecipeShell::Posix);
    let template = format!("printf '%s' $(printf %s \"$(printf %s \"`printf x`\")\") {INS_TOKEN}");
    let actual = interpolate_script_with_bindings(&template, &bindings)
        .expect("a backtick that closes at its own depth must lower");
    assert_eq!(
        actual, "printf '%s' $(printf %s \"$(printf %s \"`printf x`\")\") UNQ",
        "the script level must remain unquoted after nested substitutions"
    );
}

/// Reject nesting that closes a shell region before the backtick it owns.
///
/// The saved context belongs to the region that opened the backtick. Closing
/// that region first leaves the saved state with no owner, so there is no
/// correct region to restore it into and the script must fail closed.
#[rstest]
#[case::frame_closes_before_its_backtick("printf '%s' $(printf %s `printf x) printf y` {marker}")]
#[case::backtick_closes_before_its_nested_substitution(
    "printf '%s' `printf %s $(printf x` {marker}"
)]
#[case::marker_inside_open_backtick("printf '%s' $(printf %s `printf x {marker}")]
fn script_backtick_nesting_that_crosses_its_owner_is_rejected(#[case] template: &str) {
    let command = template.replace("{marker}", INS_TOKEN);
    let error = interpolate_script_with_bindings(&command, &context_bindings(RecipeShell::Posix))
        .expect_err("mismatched backtick and $() nesting must fail closed");
    assert!(
        matches!(error, IrGenError::InvalidCommand { .. }),
        "expected an invalid-command error for {command:?}, got {error:?}"
    );
}
