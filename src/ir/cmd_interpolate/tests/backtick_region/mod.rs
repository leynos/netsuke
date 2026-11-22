//! Script-mode regressions for backtick quote-state ownership (#1000).
//!
//! A backtick region saves the quote context of the shell region that opened
//! it. These cases pin that the closing backtick restores that region rather
//! than the script-level field, which would make a later marker use the wrong
//! path encoding, and that nesting which closes a region before its backtick
//! fails closed. They also pin the body's own quoting and escape rules.

use super::*;
use cases::Case;
use rstest::rstest;

mod cases;

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

/// Name the placeholder a case lowers.
#[derive(Clone, Copy)]
enum Marker {
    /// The `$in` marker.
    Ins,
    /// The `$out` marker.
    Outs,
}

impl Marker {
    /// Return the source token this marker is written as.
    const fn token(self) -> &'static str {
        match self {
            Self::Ins => INS_TOKEN,
            Self::Outs => OUTS_TOKEN,
        }
    }

    /// Return the sentinel the binding emits for `encoding`.
    const fn sentinel(self, encoding: Encoding) -> &'static str {
        match (self, encoding) {
            (Self::Ins, Encoding::Unquoted) => "UNQ",
            (Self::Ins, Encoding::DoubleQuoted) => "DBL",
            (Self::Outs, Encoding::Unquoted) => "OUNQ",
            (Self::Outs, Encoding::DoubleQuoted) => "ODBL",
        }
    }
}

/// Name the shell quoting a lowered marker must be encoded for.
#[derive(Clone, Copy)]
enum Encoding {
    /// The marker sits outside quotes.
    Unquoted,
    /// The marker sits inside double quotes.
    DoubleQuoted,
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

/// State what lowering must do with one case.
#[derive(Clone, Copy)]
enum Outcome {
    /// Lower successfully, encoding the marker for this context.
    Lowers(Encoding),
    /// Fail closed with `IrGenError::InvalidCommand`.
    Rejects,
}

/// Encode each marker after a backtick for the region that owns it.
///
/// The enclosing substitution opens at script level, so the marker after the
/// backtick belongs to that script region: outside quotes when the outer
/// string is unquoted, inside them when the whole substitution sits in one.
#[rstest]
#[case::ins_unquoted_outer_unquoted_body(
    RecipeShell::Posix,
    Marker::Ins,
    QuoteLayout::NoQuotes,
    Encoding::Unquoted
)]
#[case::ins_unquoted_outer_quoted_body(
    RecipeShell::Posix,
    Marker::Ins,
    QuoteLayout::BodyQuoted,
    Encoding::Unquoted
)]
#[case::ins_quoted_outer_unquoted_body(
    RecipeShell::Posix,
    Marker::Ins,
    QuoteLayout::OuterQuoted,
    Encoding::DoubleQuoted
)]
#[case::ins_quoted_outer_quoted_body(
    RecipeShell::Posix,
    Marker::Ins,
    QuoteLayout::BothQuoted,
    Encoding::DoubleQuoted
)]
#[case::outs_unquoted_outer_quoted_body(
    RecipeShell::Posix,
    Marker::Outs,
    QuoteLayout::BodyQuoted,
    Encoding::Unquoted
)]
#[case::outs_quoted_outer_unquoted_body(
    RecipeShell::Posix,
    Marker::Outs,
    QuoteLayout::OuterQuoted,
    Encoding::DoubleQuoted
)]
#[case::bash_unquoted_outer_quoted_body(
    RecipeShell::Bash,
    Marker::Ins,
    QuoteLayout::BodyQuoted,
    Encoding::Unquoted
)]
#[case::bash_quoted_outer_quoted_body(
    RecipeShell::Bash,
    Marker::Outs,
    QuoteLayout::BothQuoted,
    Encoding::DoubleQuoted
)]
fn script_marker_after_backtick_in_command_substitution_uses_owning_region(
    #[case] shell: RecipeShell,
    #[case] marker: Marker,
    #[case] layout: QuoteLayout,
    #[case] expected: Encoding,
) {
    let substitution = format!("$(printf %s {})", layout.body());
    let token = marker.token();
    let template = if layout.is_outer_quoted() {
        format!("printf '%s' \"{substitution} {token}\"")
    } else {
        format!("printf '%s' {substitution} {token}")
    };

    let actual = interpolate_script_with_bindings(&template, &context_bindings(shell))
        .expect("a backtick that closes in its owning $() must lower");
    assert_eq!(
        actual,
        template.replace(token, marker.sentinel(expected)),
        "a marker after {template:?} must use the owning region's encoding"
    );
}

/// Lower or reject one script shape and pin the marker's encoding.
///
/// Each case names a shell rule that a backtick body exercises: which
/// delimiters its own quoting protects, which escapes it honours, and which
/// nesting it forbids. A lowering case must leave the marker after the
/// substitution encoded for the region that owns it; a rejection case must
/// fail closed with `IrGenError::InvalidCommand` rather than guess.
#[rstest]
#[case::escaped_backtick(Case::EscapedBacktickInQuotedBody)]
#[case::escaped_backtick_nested_frame(Case::EscapedBacktickInNestedFrame)]
#[case::escaped_parenthesis(Case::EscapedParenthesisInUnquotedBody)]
#[case::escaped_marker(Case::EscapedMarkerInUnquotedBody)]
#[case::nested_substitution(Case::NestedSubstitutionInQuotedBody)]
#[case::double_backslash_parenthesis(Case::DoubleBackslashBeforeParenthesis)]
#[case::double_backslash_quote(Case::DoubleBackslashBeforeQuote)]
#[case::double_backslash_single_quotes(Case::DoubleBackslashInSingleQuotes)]
#[case::double_backslash_single_quotes_parenthesis(
    Case::DoubleBackslashInSingleQuotesBeforeParenthesis
)]
#[case::double_backslash_nested_frame_single_quotes(Case::DoubleBackslashInNestedFrameSingleQuotes)]
#[case::backslash_before_body_quote(Case::BackslashBeforeBodyQuote)]
#[case::quoted_parenthesis_single(Case::QuotedParenthesisInSingleQuotedBody)]
#[case::quoted_parenthesis_double(Case::QuotedParenthesisInDoubleQuotedBody)]
#[case::continuation_unquoted(Case::ContinuationInUnquotedBody)]
#[case::continuation_double_quoted(Case::ContinuationInDoubleQuotedBody)]
#[case::literal_newline_single_quoted(Case::LiteralNewlineInSingleQuotedBody)]
#[case::top_level_unquoted(Case::TopLevelBacktickUnquoted)]
#[case::top_level_after_double_quoted(Case::TopLevelBacktickAfterDoubleQuoted)]
#[case::top_level_inside_double_quotes(Case::TopLevelBacktickInsideDoubleQuotes)]
#[case::substitution_closes_first(Case::SubstitutionInsideBacktickClosesFirst)]
#[case::two_level_substitution(Case::TwoLevelSubstitutionInsideBacktick)]
#[case::frame_closes_before_backtick(Case::FrameClosesBeforeBacktick)]
#[case::backtick_closes_before_substitution(Case::BacktickClosesBeforeSubstitution)]
#[case::marker_inside_open_backtick(Case::MarkerInsideOpenBacktick)]
fn script_body_rules_follow_the_owning_shell_region(#[case] case: Case) {
    let bindings = context_bindings(RecipeShell::Posix);
    let command = case.command();
    match case.outcome() {
        Outcome::Lowers(encoding) => {
            let actual = interpolate_script_with_bindings(&command, &bindings)
                .unwrap_or_else(|error| panic!("{} must lower: {error:?}", case.description()));
            assert_eq!(
                actual,
                command.replace(INS_TOKEN, Marker::Ins.sentinel(encoding)),
                "{} must encode the marker for its own region",
                case.description()
            );
        }
        Outcome::Rejects => {
            let error = interpolate_script_with_bindings(&command, &bindings)
                .expect_err("this nesting must fail closed");
            assert!(
                matches!(error, IrGenError::InvalidCommand { .. }),
                "{} must fail closed, got {error:?}",
                case.description()
            );
        }
    }
}
