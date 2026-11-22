//! Name the heredoc syntax that the POSIX tracker refuses to model.
//!
//! Delimiter parsing and body activation both fail closed, and each refusal is
//! reported through the same invalid-command diagnostic. Keeping the reasons
//! beside their descriptions makes the words a user sees traceable to the
//! check that produced them.

/// Identify the shell region that declared a pending heredoc.
///
/// A heredoc body belongs to the command line that declared it, so the body
/// must start at that command line's newline. A unique identifier per open
/// `$( )` or backtick region lets the queue check that the newline it is
/// offered belongs to the same region that queued the declaration. The outer
/// script is region zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ir::cmd_interpolate) struct FrameId(usize);

impl FrameId {
    /// The region that encloses every substitution frame.
    pub(in crate::ir::cmd_interpolate) const OUTER: Self = Self(0);

    /// Reserve the next identifier from a monotonic counter.
    ///
    /// Nesting depth would not do: a declaration whose frame has closed is
    /// owned by an identifier no longer on the stack, and depth alone could
    /// make it collide with a later frame at the same depth.
    pub(in crate::ir::cmd_interpolate) const fn allocate(next: &mut usize) -> Self {
        let frame = Self(*next);
        *next += 1;
        frame
    }
}

/// Classify what a newline is part of when a pending heredoc could start.
///
/// Only an unquoted newline of the declaring command line is a body boundary.
/// A quoted newline is ordinary argument text the shell keeps reading through,
/// while the other variants fall in regions whose end the tracker cannot
/// correlate with the declaring command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ir::cmd_interpolate) enum NewlineContext {
    /// An unquoted newline that ends the declaring command line.
    CommandLine,
    /// A newline that is part of a quoted argument.
    QuotedArgument,
    /// A newline inside a backtick substitution body.
    BacktickBody,
    /// A newline inside a `$(( ))` arithmetic expansion.
    ArithmeticExpansion,
    /// A newline inside an open `${ }` expansion.
    ParameterExpansion,
}

/// Describe one newline offered to the pending-heredoc queue.
#[derive(Clone, Copy, Debug)]
pub(in crate::ir::cmd_interpolate) struct HeredocBoundary {
    /// The shell region the traversal is in at the newline.
    pub(in crate::ir::cmd_interpolate) frame: FrameId,
    /// What the newline is part of.
    pub(in crate::ir::cmd_interpolate) context: NewlineContext,
}

impl HeredocBoundary {
    /// Record the region and context of one newline.
    pub(in crate::ir::cmd_interpolate) const fn new(
        frame: FrameId,
        context: NewlineContext,
    ) -> Self {
        Self { frame, context }
    }
}

/// Explain why a pending heredoc body cannot start at an offered newline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ir::cmd_interpolate) enum HeredocActivationRejection {
    /// The newline is in a different shell region from the declaration.
    ForeignFrame,
    /// The newline is inside a backtick substitution body.
    BacktickNewline,
    /// The newline is inside an arithmetic expansion.
    ArithmeticNewline,
    /// The newline is inside an open parameter expansion.
    ParameterExpansionNewline,
    /// The declaring line carries an ANSI-C quote the POSIX rules misread.
    AnsiCQuoting,
}

impl HeredocActivationRejection {
    /// Describe the rejected boundary for the interpolation diagnostic.
    pub(in crate::ir::cmd_interpolate) const fn description(self) -> &'static str {
        match self {
            Self::ForeignFrame => {
                "unsupported heredoc boundary: declaration and command line span regions"
            }
            Self::BacktickNewline => "unsupported heredoc boundary: command substitution newline",
            Self::ArithmeticNewline => "unsupported heredoc boundary: arithmetic expansion newline",
            Self::ParameterExpansionNewline => {
                "unsupported heredoc boundary: parameter expansion newline"
            }
            Self::AnsiCQuoting => "unsupported heredoc boundary: ANSI-C quoting",
        }
    }
}

/// Explain why a heredoc delimiter word cannot be tracked safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ir::cmd_interpolate) enum DelimiterRejection {
    /// No delimiter word follows `<<` or `<<-`.
    MissingDelimiter,
    /// A quote opened in the delimiter word is never closed.
    UnclosedQuote,
    /// A backslash at the end of the word has no escaped character.
    TrailingBackslash,
    /// An unquoted backslash-newline continues the delimiter word.
    LineContinuation,
    /// A dollar sign appears outside single quotes.
    UnsupportedExpansion,
    /// A backtick appears outside single quotes.
    UnsupportedCommandSubstitution,
    /// Rendered path encoding stopped representing one delimited word.
    RenderedDelimiterInvalid,
}

impl DelimiterRejection {
    /// Describe the rejected syntax for the interpolation diagnostic.
    pub(in crate::ir::cmd_interpolate) const fn description(self) -> &'static str {
        match self {
            Self::MissingDelimiter => "unsupported heredoc delimiter syntax: missing delimiter",
            Self::UnclosedQuote => "unsupported heredoc delimiter syntax: unclosed quote",
            Self::TrailingBackslash => "unsupported heredoc delimiter syntax: trailing backslash",
            Self::LineContinuation => "unsupported heredoc delimiter syntax: line continuation",
            Self::UnsupportedExpansion => "unsupported heredoc delimiter syntax: expansion",
            Self::UnsupportedCommandSubstitution => {
                "unsupported heredoc delimiter syntax: command substitution"
            }
            Self::RenderedDelimiterInvalid => {
                "unsupported heredoc delimiter syntax: rendered delimiter"
            }
        }
    }
}
