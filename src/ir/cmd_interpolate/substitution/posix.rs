//! Resolve POSIX shell characters that decide whether a marker may be lowered.
//!
//! Heredoc declarations, comments, and backslash escapes are inert or
//! rejected before the shared traversal reaches marker handling, so they are
//! resolved first. Keeping them in a sibling module holds the parent within
//! the repository's module-size cap.

use super::super::invalid_heredoc_boundary_error;
use super::super::invalid_heredoc_delimiter_error;
use super::super::posix_lexical::{
    EscapeBeforeMarker, HeredocBoundary, HeredocDeclaration, InertCharacterOutcome, NewlineContext,
    PosixCharacter, SourceChar, backslash_escapes_marker,
};
use super::{
    IrGenError, MarkerProtection, RejectionReason, SubstitutionTraversal, invalid_command_error,
};

impl SubstitutionTraversal<'_, '_> {
    /// Preserve POSIX shell syntax while lowering context-safe recipe markers.
    ///
    /// A newline that ends a comment is offered to the pending-heredoc queue
    /// like any other: it is the command-line boundary the shell starts a
    /// declared body at.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when a heredoc delimiter or escape is
    /// untrackable, or when a pending body cannot start at an offered newline.
    pub(super) fn append_posix_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<usize, IrGenError> {
        if let Some(next) = self.close_backtick_comment(pos, ch) {
            return Ok(next);
        }
        match self.copy_inert_character(pos, ch) {
            Some(InertCharacterOutcome::CommentEnded) => {
                self.start_pending_heredoc_after(self.newline_context())?;
                return Ok(pos + 1);
            }
            Some(InertCharacterOutcome::Consumed) => return Ok(pos + 1),
            Some(InertCharacterOutcome::NotInert) | None => {}
        }
        self.observe_case_word(pos, ch);
        if let Some(next) = self.append_escaped_character(pos, ch)? {
            return Ok(next);
        }
        if let Some(next) = self.append_unquoted_posix_lexical_character(pos, ch)? {
            return Ok(next);
        }
        if ch == '`' && !self.matches_single_quote_context() {
            self.toggle_backtick_region();
            self.output.push(ch);
            return Ok(pos + 1);
        }
        let next = self.append_contextual_character(pos, ch, Self::posix_marker_protection)?;
        if ch == '\n' {
            self.start_pending_heredoc_after(self.newline_context())?;
        }
        Ok(next)
    }

    /// Describe what the newline at the current position is part of.
    ///
    /// Only an unquoted newline in the region that declared the heredoc is a
    /// body boundary. The other answers let activation fail closed instead of
    /// searching for a terminator in text the shell still expands.
    fn newline_context(&self) -> NewlineContext {
        if self.in_backticks {
            return NewlineContext::BacktickBody;
        }
        if self.is_in_arithmetic_expansion() {
            return NewlineContext::ArithmeticExpansion;
        }
        if self.posix_lexical.is_in_parameter_expansion() {
            return NewlineContext::ParameterExpansion;
        }
        if !self.matches_unquoted_quote_context() {
            return NewlineContext::QuotedArgument;
        }
        NewlineContext::CommandLine
    }

    /// Start the queued heredoc body if this newline is its command-line end.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when a queued declaration cannot be
    /// placed at the newline's region or context.
    fn start_pending_heredoc_after(&mut self, context: NewlineContext) -> Result<(), IrGenError> {
        let boundary = HeredocBoundary::new(self.posix_lexical.current_frame_id(), context);
        self.posix_lexical
            .activate_pending_heredoc(boundary)
            .map_err(|reason| invalid_heredoc_boundary_error(self.template.to_owned(), reason))
    }

    /// End a comment at the backtick that closes a legacy substitution.
    ///
    /// A backtick body is parsed as its own command region, so `#` inside it
    /// starts a comment that ends with the substitution rather than with the
    /// outer line. Letting comment copying run through the closing backtick
    /// leaves the traversal inside the region, so the heredoc declared later on
    /// that line is never queued.
    fn close_backtick_comment(&mut self, pos: usize, ch: char) -> Option<usize> {
        if !self.in_backticks
            || !self
                .posix_lexical
                .backtick_closes_comment(self.chars, pos, SourceChar::new(ch))
        {
            return None;
        }
        self.posix_lexical.end_comment();
        self.end_case_comment();
        self.toggle_backtick_region();
        self.output.push(ch);
        Some(pos + 1)
    }

    /// Copy a character the scanner treats as inert, ending any comment it closes.
    ///
    /// Comment text and heredoc bodies are copied without inspection. The
    /// newline that ends a comment is reported back to the caller, which owns
    /// the same pending-heredoc decision an ordinary newline takes.
    fn copy_inert_character(&mut self, pos: usize, ch: char) -> Option<InertCharacterOutcome> {
        let mut character = PosixCharacter {
            chars: self.chars,
            pos,
            ch,
            output: &mut self.output,
        };
        let outcome = self.posix_lexical.append_inert_character(&mut character);
        if matches!(outcome, InertCharacterOutcome::CommentEnded) {
            self.end_case_comment();
        }
        match outcome {
            InertCharacterOutcome::Consumed | InertCharacterOutcome::CommentEnded => Some(outcome),
            InertCharacterOutcome::NotInert => None,
        }
    }

    /// Preserve unquoted POSIX heredoc declarations and comments.
    ///
    /// An arithmetic body expands numbers, so `<<` there is a shift operator
    /// rather than a heredoc declaration and `#` is arithmetic text rather than
    /// a comment; neither is tracked.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when a heredoc delimiter would lower a
    /// marker directly after an active escape, or when the delimiter word uses
    /// syntax the tracker cannot model. Continuing would let marker lowering
    /// resume inside text the shell still treats as heredoc data, where command
    /// substitutions in inserted paths remain executable.
    fn append_unquoted_posix_lexical_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if !self.matches_unquoted_quote_context() || self.is_in_arithmetic_expansion() {
            return Ok(None);
        }
        self.posix_lexical
            .note_parameter_expansion(self.chars, pos, SourceChar::new(ch));
        let mut character = PosixCharacter {
            chars: self.chars,
            pos,
            ch,
            output: &mut self.output,
        };
        let declaration = self
            .posix_lexical
            .append_heredoc_declaration(&mut character, self.bindings)
            .map_err(|EscapeBeforeMarker| {
                invalid_command_error(
                    self.template.to_owned(),
                    RejectionReason::EscapeBeforeMarker,
                )
            })?;
        match declaration {
            HeredocDeclaration::NotADeclaration => {}
            HeredocDeclaration::Copied(next) => return Ok(Some(next)),
            HeredocDeclaration::Rejected(reason) => {
                return Err(invalid_heredoc_delimiter_error(
                    self.template.to_owned(),
                    reason,
                ));
            }
        }
        if self
            .posix_lexical
            .starts_comment(self.chars, pos, SourceChar::new(ch))
        {
            self.posix_lexical.begin_comment();
            self.begin_case_comment();
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        Ok(None)
    }

    /// Classify POSIX backticks and command substitutions as protected regions.
    pub(super) const fn posix_marker_protection(&self, _pos: usize) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            self.in_backticks || self.is_in_command_substitution(),
        )
    }

    /// Preserve a POSIX escaped character without interpreting its quote meaning.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the escape would modify a marker
    /// lowered immediately after it.
    fn append_escaped_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if ch != '\\' || self.matches_single_quote_context() {
            return Ok(None);
        }
        if backslash_escapes_marker(self.chars, pos, false, None) {
            return Err(invalid_command_error(
                self.template.to_owned(),
                RejectionReason::EscapeBeforeMarker,
            ));
        }
        let Some(next) = self.chars.get(pos + 1).copied() else {
            return Ok(None);
        };
        self.output.push(ch);
        self.output.push(next);
        Ok(Some(pos + 2))
    }
}
