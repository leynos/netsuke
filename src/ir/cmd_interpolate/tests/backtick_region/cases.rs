//! Typed table of the backtick body shapes the regressions cover.
//!
//! Each variant renders one script template and states what lowering must do
//! with it, so the case functions in the parent module stay small and the
//! names state the shell rule under test.

use super::super::*;
use super::{Encoding, Outcome};

/// Name one script shape that the backtick ownership rules must handle.
#[derive(Clone, Copy)]
pub(super) enum Case {
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
    /// A `\\` pair leaves a backslash that escapes the body's own parenthesis.
    DoubleBackslashBeforeParenthesis,
    /// A `\\` pair leaves a backslash that escapes a body quote.
    DoubleBackslashBeforeQuote,
    /// A `\\` pair inside single quotes leaves a literal backslash only.
    DoubleBackslashInSingleQuotes,
    /// A backslash-dollar exposes a command substitution to the body's parse.
    ExposedSubstitutionAfterEscapedDollar,
    /// A comment inside the body hides the parenthesis on its line.
    CommentInsideBodyHidesParenthesis,
    /// A body parenthesis must not stand in for the group closer.
    BodyParenthesisInsideGroupedSubstitution,
    /// A `#` inside `${ }` in a nested frame of the body is not a comment.
    ParameterExpansionHashInNestedFrame,
    /// A single-quoted `\\` pair before a quoted parenthesis is literal.
    DoubleBackslashInSingleQuotesBeforeParenthesis,
    /// A single-quoted `\\` pair in a `$()` frame in the body is literal.
    DoubleBackslashInNestedFrameSingleQuotes,
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
    pub(super) fn command(self) -> String {
        self.body_command()
            .unwrap_or_else(|| self.nesting_command())
    }

    /// Render a body shape whose delimiters or escapes are under test.
    fn body_command(self) -> Option<String> {
        let command = match self {
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
            Self::DoubleBackslashBeforeParenthesis => {
                format!(r"printf '<%s>\n' $(printf %s `printf %s \\)`) {INS_TOKEN}")
            }
            Self::DoubleBackslashBeforeQuote => {
                format!(r"printf '%s' `printf %s \\'` {INS_TOKEN}")
            }
            Self::DoubleBackslashInSingleQuotes => {
                format!(r"printf '<%s>\n' `printf %s '\\'` {INS_TOKEN}")
            }
            Self::ExposedSubstitutionAfterEscapedDollar => {
                format!(r#"printf '%s' $(printf %s `printf %s "\$(printf %s ")")"`) {INS_TOKEN}"#)
            }
            Self::CommentInsideBodyHidesParenthesis => {
                format!("printf '%s' $(printf %s `printf x # )\n`) {INS_TOKEN}")
            }
            Self::BodyParenthesisInsideGroupedSubstitution => {
                format!(r#"printf '%s' "$( (printf %s `printf x)`) {INS_TOKEN} )""#)
            }
            Self::ParameterExpansionHashInNestedFrame => {
                format!(r#"printf '%s' `printf '%s' "$(printf '%s' ${{v# #}})"` {INS_TOKEN}"#)
            }
            Self::DoubleBackslashInSingleQuotesBeforeParenthesis => {
                format!(r"printf '<%s>\n' $(printf %s `printf %s '\\' ')'`) {INS_TOKEN}")
            }
            Self::DoubleBackslashInNestedFrameSingleQuotes => {
                format!(r#"printf '<%s>\n' `printf %s "$(printf %s '\\')"` {INS_TOKEN}"#)
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
            _ => return None,
        };
        Some(command)
    }

    /// Render a backtick or `$()` nesting shape.
    fn nesting_command(self) -> String {
        match self {
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
            // Every other variant is rendered by `body_command`.
            _ => self.body_command().unwrap_or_default(),
        }
    }

    /// State what lowering must do with this case.
    pub(super) const fn outcome(self) -> Outcome {
        match self {
            Self::EscapedMarkerInUnquotedBody
            | Self::BackslashBeforeBodyQuote
            | Self::FrameClosesBeforeBacktick
            | Self::BacktickClosesBeforeSubstitution
            | Self::MarkerInsideOpenBacktick
            | Self::BodyParenthesisInsideGroupedSubstitution => Outcome::Rejects,
            Self::TopLevelBacktickInsideDoubleQuotes => Outcome::Lowers(Encoding::DoubleQuoted),
            _ => Outcome::Lowers(Encoding::Unquoted),
        }
    }

    /// Describe this case in a failure message.
    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::EscapedBacktickInQuotedBody => "an escaped backtick in a quoted body",
            Self::EscapedBacktickInNestedFrame => "an escaped backtick in a nested frame",
            Self::EscapedParenthesisInUnquotedBody => "an escaped ) in an unquoted body",
            Self::EscapedMarkerInUnquotedBody => "a backslash before a marker",
            Self::NestedSubstitutionInQuotedBody => "a $() inside a double-quoted backtick body",
            Self::DoubleBackslashBeforeParenthesis => "a surviving backslash before a body )",
            Self::DoubleBackslashBeforeQuote => "a surviving backslash before a body quote",
            Self::DoubleBackslashInSingleQuotes => "a \\\\ pair inside body single quotes",
            Self::ExposedSubstitutionAfterEscapedDollar => {
                "a substitution exposed by an escaped dollar sign"
            }
            Self::CommentInsideBodyHidesParenthesis => "a comment hiding a body parenthesis",
            Self::BodyParenthesisInsideGroupedSubstitution => {
                "a body parenthesis inside a grouped substitution"
            }
            Self::ParameterExpansionHashInNestedFrame => {
                "a # inside ${ } in a nested frame of the body"
            }
            Self::DoubleBackslashInSingleQuotesBeforeParenthesis => {
                "a single-quoted \\\\ pair before a quoted parenthesis"
            }
            Self::DoubleBackslashInNestedFrameSingleQuotes => {
                "a single-quoted \\\\ pair in a nested frame"
            }
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
// weave: run 'weave explain src/ir/cmd_interpolate/tests/backtick_region/cases.rs' for per-hunk detail, 'weave check' to verify your resolution
