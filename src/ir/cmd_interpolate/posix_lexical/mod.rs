//! Preserve POSIX comments and heredoc data during recipe interpolation.
//!
//! The interpolation traversal must not treat quote-like characters inside
//! inert shell regions as syntax. This module recognises just enough POSIX
//! lexical structure to retain those regions byte-for-byte while the caller
//! lowers markers in executable shell text.

use std::collections::VecDeque;

use super::find_substitution;

mod ansi_c;

pub(super) use ansi_c::AnsiCCharacter;

mod declaration;

mod delimiter;

mod diagnostics;

mod parameter_expansion;

#[cfg(test)]
mod tests;

pub(super) use diagnostics::{
    DelimiterRejection, HeredocActivationRejection, HeredocBoundary, NewlineContext,
};

pub(in crate::ir::cmd_interpolate) use declaration::HeredocDeclaration;

use declaration::{HeredocBody, HeredocDelimiter};
use diagnostics::FrameId;

/// Describe what copying a character from an inert region did.
pub(super) enum InertCharacterOutcome {
    /// No inert region was active; the caller still owns the character.
    NotInert,
    /// The character belonged to a comment or heredoc body and was copied.
    Consumed,
    /// The character was the newline that ended a shell comment.
    CommentEnded,
}

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
    /// Allocate the next unique shell-region identifier.
    next_frame_id: usize,
    /// Record the shell regions currently open, innermost last.
    frame_stack: Vec<FrameId>,
    /// Record whether a live `$` may open an ANSI-C string at the next quote.
    pending_ansi_c_opener: bool,
    /// Record whether the previous significant unquoted character was a `$`
    /// awaiting the second half of a `$$` pair.
    unpaired_dollar: bool,
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

/// Report whether the ANSI-C quote at `pos` is read differently by POSIX rules.
///
/// Bash reads `$'…'` as an ANSI-C string, where a backslash escapes the next
/// character. POSIX single-quote rules instead end the string at the first
/// `'`, so an escape inside the text leaves the two readings with different
/// quote state at later newlines. Text with no escape ends at the same quote
/// under both readings, so only the ambiguous form is reported. An
/// unterminated string also returns `false`: both readings stay inside it.
///
/// Bash removes an unquoted backslash-newline continuation before it reads the
/// character that follows, so `$`, a continuation, and the opening quote name
/// the same ANSI-C string. The scan steps over those pairs first; an escaped
/// backslash stops it, because the newline after an escape pair is a real
/// newline rather than a continuation.
pub(super) fn ansic_quote_is_ambiguous(chars: &[char], pos: usize) -> bool {
    if chars.get(pos) != Some(&'$') {
        return false;
    }
    let mut quote_pos = pos + 1;
    while chars.get(quote_pos) == Some(&'\\') && chars.get(quote_pos + 1) == Some(&'\n') {
        quote_pos += 2;
    }
    if chars.get(quote_pos) != Some(&'\'') {
        return false;
    }
    let mut saw_escape = false;
    for &ch in chars.get(quote_pos + 1..).into_iter().flatten() {
        match ch {
            '\\' => saw_escape = true,
            '\'' => return saw_escape,
            _ => {}
        }
    }
    false
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
            next_frame_id: 1,
            frame_stack: Vec::new(),
            pending_ansi_c_opener: false,
            unpaired_dollar: false,
        }
    }

    /// Return the innermost open shell region, or the outer script.
    ///
    /// A declaration queued while this identifier is current belongs to that
    /// region's command line. The comparison at activation time is what stops
    /// an outer declaration from starting at a nested region's newline, or a
    /// declaration whose frame has already closed from starting outside it.
    pub(in crate::ir::cmd_interpolate) fn current_frame_id(&self) -> FrameId {
        self.frame_stack.last().copied().unwrap_or(FrameId::OUTER)
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
    ///
    /// The newline that ends a comment is reported rather than acted on: it is
    /// a command-line boundary, so both traversals must offer it to the same
    /// activation decision as an ordinary newline. A body terminator still
    /// starts the next queued body directly, because chained declarations share
    /// one command line and their bodies follow one another without another
    /// boundary.
    pub(super) fn append_inert_character(
        &mut self,
        character: &mut PosixCharacter<'_, '_>,
    ) -> InertCharacterOutcome {
        if self.in_comment {
            character.output.push(character.ch);
            if character.ch == '\n' {
                self.in_comment = false;
                return InertCharacterOutcome::CommentEnded;
            }
            return InertCharacterOutcome::Consumed;
        }
        if self.heredoc_body.is_none() {
            return InertCharacterOutcome::NotInert;
        }
        self.reset_ansi_c_lookbehind();
        let Some(body) = self.heredoc_body.as_mut() else {
            return InertCharacterOutcome::NotInert;
        };
        character.output.push(character.ch);
        if body.consume(SourceChar::new(character.ch)) {
            self.heredoc_body = None;
            self.begin_next_heredoc();
        }
        InertCharacterOutcome::Consumed
    }

    /// Begin a POSIX comment after its opening number sign is copied.
    pub(super) const fn begin_comment(&mut self) {
        self.in_comment = true;
        self.reset_ansi_c_lookbehind();
    }

    /// Decide whether a pending heredoc body may start at one newline.
    ///
    /// POSIX starts the body after the newline that ends the command line
    /// declaring it. A quoted newline is still part of that command line, so
    /// the queue waits for the real boundary and the traversal's quote state
    /// stays accurate. A newline in another region, or in a backtick,
    /// arithmetic, or parameter-expansion body, cannot be correlated with the
    /// declaring line; activating there would search for the terminator in
    /// text the shell still expands, so the traversal fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`HeredocActivationRejection`] when a queued declaration cannot
    /// be placed at the offered boundary.
    pub(super) fn activate_pending_heredoc(
        &mut self,
        boundary: HeredocBoundary,
    ) -> Result<(), HeredocActivationRejection> {
        let Some(delimiter) = self.pending_heredocs.front() else {
            return Ok(());
        };
        match (boundary.frame == delimiter.owner, boundary.context) {
            // A quoted newline never ends a command line, so the body keeps
            // waiting for the closing quote's newline.
            (true, NewlineContext::QuotedArgument) => Ok(()),
            // In another region the tracker cannot correlate that region's
            // lines with the declaring command line, whether quoted or not.
            (false, _) => Err(HeredocActivationRejection::ForeignFrame),
            (true, NewlineContext::CommandLine) => {
                self.begin_next_heredoc();
                Ok(())
            }
            (true, NewlineContext::BacktickBody) => {
                Err(HeredocActivationRejection::BacktickNewline)
            }
            (true, NewlineContext::ArithmeticExpansion) => {
                Err(HeredocActivationRejection::ArithmeticNewline)
            }
            (true, NewlineContext::ParameterExpansion) => {
                Err(HeredocActivationRejection::ParameterExpansionNewline)
            }
        }
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
pub(super) fn is_unescaped_character(chars: &[char], pos: usize) -> bool {
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
