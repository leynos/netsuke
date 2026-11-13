//! Preserve POSIX comments and heredoc data during recipe interpolation.
//!
//! The interpolation traversal must not treat quote-like characters inside
//! inert shell regions as syntax. This module recognises just enough POSIX
//! lexical structure to retain those regions byte-for-byte while the caller
//! lowers markers in executable shell text.

use std::collections::VecDeque;

use super::find_substitution;

mod declaration;

mod delimiter;

mod parameter_expansion;

#[cfg(test)]
mod tests;

/// Track POSIX regions whose contents are not executable shell syntax.
pub(super) struct PosixLexicalState {
    /// Record whether text through the next newline is a shell comment.
    in_comment: bool,
    /// Retain heredoc delimiters declared on the current shell command line.
    pending_heredocs: VecDeque<HeredocDelimiter>,
    /// Retain the currently copied heredoc body, if one has begun.
    heredoc_body: Option<HeredocBody>,
    /// Count the `${` parameter expansions left open in plain text.
    parameter_expansion_depth: usize,
    /// Save that count for each enclosing command-substitution region.
    enclosing_parameter_expansion_depths: Vec<usize>,
    /// Record the `)` that closed the most recent command substitution.
    closed_command_substitution: Option<usize>,
}

/// Retain one heredoc delimiter and the rules that select its terminator.
struct HeredocDelimiter {
    /// Store the delimiter after POSIX quote removal.
    text: String,
    /// Record whether the delimiter suppresses expansion in the body.
    is_quoted: bool,
    /// Record whether `<<-` strips leading tabs before delimiter matching.
    strips_leading_tabs: bool,
}

/// Retain state needed to recognise a terminating heredoc line.
struct HeredocBody {
    /// Retain the declaration that selected this body.
    delimiter: HeredocDelimiter,
    /// Accumulate the logical line the shell compares with the delimiter.
    line: String,
    /// Record whether the next character begins a logical line.
    at_line_start: bool,
}

/// Describe the outcome of examining characters that may open a heredoc.
pub(super) enum HeredocDeclaration {
    /// The characters do not begin a heredoc or here-string operator.
    NotADeclaration,
    /// The operator was copied; scanning resumes at this source position.
    Copied(usize),
    /// The declaration uses delimiter syntax the tracker cannot model.
    Rejected(DelimiterRejection),
}

/// Explain why a heredoc delimiter word cannot be tracked safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DelimiterRejection {
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
    pub(super) const fn description(self) -> &'static str {
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

/// Report that an escape would modify a marker lowered immediately after it.
///
/// The heredoc renderer is a pure string transformation, so it states this
/// condition with a private unit error that both recipe traversals convert into
/// the public interpolation diagnostic.
#[derive(Debug, Clone, Copy)]
pub(super) struct EscapeBeforeMarker;

/// Report whether the backslash at `pos` escapes a marker that follows it.
///
/// Callers only ask about a backslash they have not already consumed as an
/// escaped pair. A marker is a hazard only when the insertion site decides what
/// the backslash escapes, so single quotes and markers that end outside
/// `bound` are not rejected here.
pub(super) fn backslash_escapes_marker(
    chars: &[char],
    pos: usize,
    is_single_quoted: bool,
    bound: Option<usize>,
) -> bool {
    if is_single_quoted || chars.get(pos) != Some(&'\\') {
        return false;
    }
    let Some((_, skip)) = find_substitution(chars, pos + 1) else {
        return false;
    };
    bound.is_none_or(|end| pos + 1 + skip <= end)
}

/// Classify what a `$(` delimiter opens.
///
/// Both recipe traversals must agree on this boundary. An arithmetic body
/// evaluates numbers, so `<<` there is a shift operator rather than a heredoc
/// declaration and `#` is arithmetic text rather than a comment. Naming the
/// answer as a type keeps that agreement in one place, and stops a call site
/// from passing a Boolean whose sense it has reversed.
#[derive(Clone, Copy)]
pub(super) enum SubstitutionKind {
    /// A command substitution whose body is a command list.
    Command,
    /// A `$(( ))` arithmetic expansion, whose body is an expression.
    Arithmetic,
}

impl SubstitutionKind {
    /// Classify the `$(` delimiter at `pos` from the character that follows it.
    pub(super) fn at(chars: &[char], pos: usize) -> Self {
        if chars.get(pos + 2) == Some(&'(') {
            Self::Arithmetic
        } else {
            Self::Command
        }
    }
}

/// Classify one source character with POSIX shell word rules.
///
/// Only ASCII blanks and the shell operator set separate words; Unicode
/// whitespace is ordinary word text. Naming that rule as a type keeps the
/// distinction explicit at each call site that decides where a word ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceChar(char);

impl SourceChar {
    /// Wrap one source character for POSIX word classification.
    pub(super) const fn new(ch: char) -> Self {
        Self(ch)
    }

    /// Return this character as source text.
    pub(super) const fn as_char(self) -> char {
        self.0
    }

    /// Report whether this character is a POSIX blank: a space or a tab.
    const fn is_blank(self) -> bool {
        matches!(self.0, ' ' | '\t')
    }

    /// Report whether this character terminates an unquoted shell word.
    ///
    /// POSIX shells tokenize on blanks, newlines, and operator characters.
    /// Unicode whitespace such as NBSP, em space, vertical tab, form feed, and
    /// carriage return is ordinary word text, so treating it as a separator
    /// would end a delimiter before the shell does.
    pub(super) const fn is_word_terminator(self) -> bool {
        self.is_blank() || matches!(self.0, '\n' | ';' | '|' | '&' | '(' | ')' | '<' | '>')
    }
}

/// Group the current source character with the output it must preserve.
pub(super) struct PosixCharacter<'chars, 'output> {
    /// Borrow the source-character buffer for lexical checks.
    pub(super) chars: &'chars [char],
    /// Record the source position of the current character.
    pub(super) pos: usize,
    /// Store the source character being processed.
    pub(super) ch: char,
    /// Borrow the output being built by the interpolation traversal.
    pub(super) output: &'output mut String,
}

impl PosixLexicalState {
    /// Initialise empty lexical state for one interpolation traversal.
    pub(super) const fn new() -> Self {
        Self {
            in_comment: false,
            pending_heredocs: VecDeque::new(),
            heredoc_body: None,
            parameter_expansion_depth: 0,
            enclosing_parameter_expansion_depths: Vec::new(),
            closed_command_substitution: None,
        }
    }

    /// Record the `)` at `pos` as the closer of a command substitution.
    ///
    /// A closer stays part of the surrounding word, so the comment boundary
    /// check must not read it as a separator. The tracker owns the position
    /// because it owns the boundary decision that consults it.
    pub(super) const fn record_closed_command_substitution(&mut self, pos: usize) {
        self.closed_command_substitution = Some(pos);
    }

    /// Preserve one character from a comment or heredoc body when active.
    pub(super) fn append_inert_character(
        &mut self,
        character: &mut PosixCharacter<'_, '_>,
    ) -> bool {
        if self.in_comment {
            character.output.push(character.ch);
            if character.ch == '\n' {
                self.in_comment = false;
                self.begin_next_heredoc();
            }
            return true;
        }
        let Some(body) = self.heredoc_body.as_mut() else {
            return false;
        };
        character.output.push(character.ch);
        if body.consume(SourceChar::new(character.ch)) {
            self.heredoc_body = None;
            self.begin_next_heredoc();
        }
        true
    }

    /// Begin a POSIX comment after its opening number sign is copied.
    pub(super) const fn begin_comment(&mut self) {
        self.in_comment = true;
    }

    /// Report whether the traversal is inside a POSIX comment.
    ///
    /// Comment text is copied verbatim rather than interpreted, so a caller
    /// that tracks token state has to report the comment's end itself. Asking
    /// before copying a character distinguishes the newline that ends a comment
    /// from any other inert character.
    pub(super) const fn in_comment(&self) -> bool {
        self.in_comment
    }

    /// Begin the first queued heredoc body after its declaration line ends.
    pub(super) fn begin_pending_heredoc_after_newline(&mut self) {
        self.begin_next_heredoc();
    }

    /// Begin the next queued heredoc body, if its declaration has completed.
    fn begin_next_heredoc(&mut self) {
        let Some(delimiter) = self.pending_heredocs.pop_front() else {
            return;
        };
        self.heredoc_body = Some(HeredocBody {
            delimiter,
            line: String::new(),
            at_line_start: true,
        });
    }
}

impl HeredocBody {
    /// Record one copied body character and report whether it ends the body.
    ///
    /// An unquoted delimiter leaves the body subject to shell expansion, and
    /// the shell removes backslash-newline before it looks for the terminator.
    /// Comparing physical lines would therefore end the body early and let
    /// marker lowering resume inside text the shell still expands, so those
    /// bodies accumulate logical lines and compare only once one is complete.
    /// A quoted delimiter suppresses expansion and joining alike, so each
    /// physical line is already the unit the shell compares.
    fn consume(&mut self, ch: SourceChar) -> bool {
        if ch.as_char() == '\n' {
            return self.finish_line();
        }
        if self.is_stripped_tab(ch) {
            return false;
        }
        self.at_line_start = false;
        self.line.push(ch.as_char());
        false
    }

    /// Report whether `ch` is a leading tab that `<<-` removes before matching.
    const fn is_stripped_tab(&self, ch: SourceChar) -> bool {
        self.at_line_start && self.delimiter.strips_leading_tabs && ch.as_char() == '\t'
    }

    /// Compare the completed line and report whether the body ends here.
    fn finish_line(&mut self) -> bool {
        if !self.delimiter.is_quoted && self.remove_unescaped_trailing_backslash() {
            // The shell joins this line to the next before comparing, and only
            // a completed logical line can match the delimiter. A line holding
            // nothing but the backslash leaves the logical line empty, so the
            // next physical line still begins it and `<<-` tab stripping still
            // applies there.
            self.at_line_start = self.line.is_empty();
            return false;
        }
        let is_terminator = self.line == self.delimiter.text;
        self.line.clear();
        self.at_line_start = true;
        is_terminator
    }

    /// Remove one trailing backslash that continues the logical line.
    fn remove_unescaped_trailing_backslash(&mut self) -> bool {
        let backslashes = self
            .line
            .chars()
            .rev()
            .take_while(|character| *character == '\\')
            .count();
        if backslashes.is_multiple_of(2) {
            return false;
        }
        self.line.pop();
        true
    }
}

/// Report whether `pos` follows a boundary where POSIX permits a comment.
///
/// POSIX starts a comment only where a token may start: at the beginning of
/// input or after an unescaped separator. Every other character is word data,
/// so Unicode spaces, carriage return, vertical tab, and form feed do not
/// begin a comment and do not stop quote tracking. A `)` that closes a command
/// substitution also remains in the surrounding word, so the caller passes its
/// position as `closed_substitution` to suppress that false boundary.
fn is_shell_word_boundary(chars: &[char], pos: usize, closed_substitution: Option<usize>) -> bool {
    let Some(previous_pos) = preceding_shell_character(chars, pos) else {
        return true;
    };
    if closed_substitution == Some(previous_pos) {
        return false;
    }
    is_unescaped_character(chars, pos)
        && is_unescaped_character(chars, previous_pos)
        && chars
            .get(previous_pos)
            .is_some_and(|previous| SourceChar::new(*previous).is_word_terminator())
}

/// Find the preceding shell character after removing unquoted line continuations.
fn preceding_shell_character(chars: &[char], pos: usize) -> Option<usize> {
    let mut end = pos;
    loop {
        let previous = end.checked_sub(1)?;
        if chars.get(previous) != Some(&'\n') || is_unescaped_character(chars, previous) {
            return Some(previous);
        }
        end = previous.checked_sub(1)?;
    }
}

/// Report whether the character at `pos` is not escaped by a backslash run.
fn is_unescaped_character(chars: &[char], pos: usize) -> bool {
    chars
        .get(..pos)
        .into_iter()
        .flatten()
        .rev()
        .take_while(|character| **character == '\\')
        .count()
        .rem_euclid(2)
        == 0
}
