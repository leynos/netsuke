//! Script-mode regressions for backtick quote-state ownership (#1000).
//!
//! A backtick region saves the quote context of the shell region that opened
//! it. These cases pin that the closing backtick restores that region rather
//! than the script-level field, which would make a later marker use the wrong
//! path encoding. They also pin that nesting which closes a region before its
//! backtick fails closed instead of corrupting the enclosing quote state, and
//! that the body's own quoting and escapes decide which delimiters are data.

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

/// Name one script shape that the backtick ownership rules must handle.
#[derive(Clone, Copy)]
enum Case {
    /// A backslash escapes the backtick, so the region continues.
    EscapedBacktickInQuotedBody,
    /// A backtick escaped inside a `$()` frame in the body stays literal.
    EscapedBacktickInNestedFrame,
    /// An escaped `)` in unquoted body text is literal, not a closer.
    EscapedParenthesisInUnquotedBody,
    /// An escape that would consume a marker's first character is rejected.
    EscapedMarkerInUnquotedBody,
    /// A `$()` opened inside a quoted body closes its own frame.
    NestedSubstitutionInQuotedBody,
    /// A backslash does not escape the quote that closes a body's single quote.
    BackslashBeforeBodyQuote,
    /// A `)` inside a body's single quotes is data.
    QuotedParenthesisInSingleQuotedBody,
    /// A `)` inside a body's double quotes is data.
    QuotedParenthesisInDoubleQuotedBody,
    /// A continued newline in unquoted body text keeps the line open.
    ContinuationInUnquotedBody,
    /// A continued newline in double-quoted body text keeps the line open.
    ContinuationInDoubleQuotedBody,
    /// A literal newline in a single-quoted body does not end the command line.
    LiteralNewlineInSingleQuotedBody,
    /// A top-level backtick outside quotes restores script-level unquoted text.
    TopLevelBacktickUnquoted,
    /// A top-level backtick in a quoted word restores unquoted script text.
    TopLevelBacktickAfterDoubleQuoted,
    /// A top-level backtick inside double quotes restores that quoting.
    TopLevelBacktickInsideDoubleQuotes,
    /// A `$()` inside a backtick may close before the backtick does.
    SubstitutionInsideBacktickClosesFirst,
    /// Two `$()` levels inside a backtick restore the outermost region.
    TwoLevelSubstitutionInsideBacktick,
    /// A `$()` that closes before the backtick it owns is rejected.
    FrameClosesBeforeBacktick,
    /// A backtick that closes before the `$()` it opened is rejected.
    BacktickClosesBeforeSubstitution,
    /// A marker inside a still-open body is rejected.
    MarkerInsideOpenBacktick,
}

impl Case {
    /// Return the script template this case lowers.
    fn command(self) -> String {
        match self {
            Self::EscapedBacktickInQuotedBody => {
                format!(r"printf '%s' `printf '%s' '\`'` {INS_TOKEN}")
            }
            Self::EscapedBacktickInNestedFrame => {
                format!(r#"printf '<%s>\n' `printf %s "$(printf %s '\`')"` {INS_TOKEN}"#)
            }
            Self::EscapedParenthesisInUnquotedBody => {
                format!(r"printf '%s' $(printf %s `printf \)`) {INS_TOKEN}")
            }
            Self::EscapedMarkerInUnquotedBody => {
                format!("printf '%s' `printf %s \\{INS_TOKEN}`")
            }
            Self::NestedSubstitutionInQuotedBody => {
                format!(r#"printf '%s' `printf '%s' "$(printf x)"` {INS_TOKEN}"#)
            }
            Self::BackslashBeforeBodyQuote => {
                format!(r"printf '%s' $(printf %s `printf '%s' 'x\' )` {INS_TOKEN}")
            }
            Self::QuotedParenthesisInSingleQuotedBody => {
                format!("printf '%s' $(printf %s `printf ')'`) {INS_TOKEN}")
            }
            Self::QuotedParenthesisInDoubleQuotedBody => {
                format!("printf '%s' $(printf %s `printf \")\"`) {INS_TOKEN}")
            }
            Self::ContinuationInUnquotedBody => {
                format!("cat <<EOF `echo x \\\ny` {INS_TOKEN}\nEOF\n")
            }
            Self::ContinuationInDoubleQuotedBody => {
                format!("cat <<EOF `\"echo x \\\ny\"` {INS_TOKEN}\nEOF\n")
            }
            Self::LiteralNewlineInSingleQuotedBody => {
                format!("cat <<EOF `printf '%s' 'x\\\ny'` {INS_TOKEN}\npayload\nEOF\n")
            }
            Self::TopLevelBacktickUnquoted => format!("printf '%s' `printf x` {INS_TOKEN}"),
            Self::TopLevelBacktickAfterDoubleQuoted => {
                format!("printf '%s' \"`printf x`\" {INS_TOKEN}")
            }
            Self::TopLevelBacktickInsideDoubleQuotes => {
                format!("printf '%s' \"`printf x` {INS_TOKEN}\"")
            }
            Self::SubstitutionInsideBacktickClosesFirst => {
                format!("printf '%s' `echo $(date)` {INS_TOKEN}")
            }
            Self::TwoLevelSubstitutionInsideBacktick => {
                format!("printf '%s' $(printf %s \"$(printf %s \"`printf x`\")\") {INS_TOKEN}")
            }
            Self::FrameClosesBeforeBacktick => {
                format!("printf '%s' $(printf %s `printf x) printf y` {INS_TOKEN}")
            }
            Self::BacktickClosesBeforeSubstitution => {
                format!("printf '%s' `printf %s $(printf x` {INS_TOKEN}")
            }
            Self::MarkerInsideOpenBacktick => {
                format!("printf '%s' $(printf %s `printf x {INS_TOKEN}")
            }
        }
    }

    /// State what lowering must do with this case.
    const fn outcome(self) -> Outcome {
        match self {
            Self::EscapedMarkerInUnquotedBody
            | Self::BackslashBeforeBodyQuote
            | Self::FrameClosesBeforeBacktick
            | Self::BacktickClosesBeforeSubstitution
            | Self::MarkerInsideOpenBacktick => Outcome::Rejects,
            Self::TopLevelBacktickInsideDoubleQuotes => Outcome::Lowers(Encoding::DoubleQuoted),
            _ => Outcome::Lowers(Encoding::Unquoted),
        }
    }

    /// Describe this case in a failure message.
    const fn description(self) -> &'static str {
        match self {
            Self::EscapedBacktickInQuotedBody => "an escaped backtick in a quoted body",
            Self::EscapedBacktickInNestedFrame => {
                "an escaped backtick inside a $() frame in the body"
            }
            Self::EscapedParenthesisInUnquotedBody => "an escaped ) in an unquoted body",
            Self::EscapedMarkerInUnquotedBody => "a backslash before a marker",
            Self::NestedSubstitutionInQuotedBody => "a $() inside a double-quoted backtick body",
            Self::BackslashBeforeBodyQuote => "a backslash before a body's closing quote",
            Self::QuotedParenthesisInSingleQuotedBody => "a quoted ) in a single-quoted body",
            Self::QuotedParenthesisInDoubleQuotedBody => "a quoted ) in a double-quoted body",
            Self::ContinuationInUnquotedBody => "a line continuation in an unquoted body",
            Self::ContinuationInDoubleQuotedBody => "a line continuation in a quoted body",
            Self::LiteralNewlineInSingleQuotedBody => "a literal newline in a single-quoted body",
            Self::TopLevelBacktickUnquoted => "a top-level backtick outside quotes",
            Self::TopLevelBacktickAfterDoubleQuoted => "a quoted top-level backtick",
            Self::TopLevelBacktickInsideDoubleQuotes => "a top-level backtick inside quotes",
            Self::SubstitutionInsideBacktickClosesFirst => "a $() nested inside a backtick",
            Self::TwoLevelSubstitutionInsideBacktick => "two $() levels inside a backtick",
            Self::FrameClosesBeforeBacktick => "a $() that outlives its own backtick",
            Self::BacktickClosesBeforeSubstitution => "a backtick that closes before its $()",
            Self::MarkerInsideOpenBacktick => "a marker inside an open backtick body",
        }
    }
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
