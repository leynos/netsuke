//! Recognize POSIX heredoc declarations and comments in plain script text.
//!
//! A declaration is copied with its delimiter, so quote removal and any
//! rejection are decided once, before marker lowering resumes. The body that
//! follows stays inert until its terminator, which keeps inserted paths out of
//! text the shell expands. The helpers stay separate from the traversal so each
//! module keeps one responsibility.

use super::super::{
    IrGenError, RejectionReason, invalid_command_error, invalid_heredoc_boundary_error,
    invalid_heredoc_delimiter_error,
    posix_lexical::{
        EscapeBeforeMarker, HeredocBoundary, HeredocDeclaration, NewlineContext, PosixCharacter,
        SourceChar,
    },
};
use super::{ScriptSubstitutionTraversal, ShellQuoteContext};

impl ScriptSubstitutionTraversal<'_, '_> {
    /// Preserve unquoted POSIX heredoc declarations before their bodies become inert.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when a heredoc delimiter would lower a
    /// marker directly after an active escape, or when the delimiter word uses
    /// syntax the tracker cannot model. Continuing would let marker lowering
    /// resume inside text the shell still treats as heredoc data, where command
    /// substitutions in inserted paths remain executable.
    pub(super) fn append_unquoted_posix_lexical_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if self.is_in_arithmetic_expansion() {
            return Ok(None);
        }
        // `${ }` nesting is tracked wherever the shell reads plain expansion
        // text, including an open backtick body: a `#` inside the expansion is
        // syntax there, and reading it as a comment would swallow the rest of
        // the line, including the nested substitution's closer.
        if self.active_quote_context() == ShellQuoteContext::Unquoted {
            self.posix_lexical
                .note_parameter_expansion(self.chars, pos, SourceChar::new(ch));
        }
        if !self.is_plain_script_text() {
            return Ok(None);
        }
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
            HeredocDeclaration::NotADeclaration => Ok(None),
            HeredocDeclaration::Copied(next) => Ok(Some(next)),
            HeredocDeclaration::Rejected(reason) => Err(invalid_heredoc_delimiter_error(
                self.template.to_owned(),
                reason,
            )),
        }
    }

    /// Describe what the newline at the current position is part of.
    ///
    /// Only an unquoted newline in the region that declared the heredoc is a
    /// body boundary. A quoted newline is argument text the shell keeps
    /// reading through; the nested-region answers let activation fail closed.
    pub(super) fn newline_context(&self) -> NewlineContext {
        if self.backtick_region.is_some() {
            return NewlineContext::BacktickBody;
        }
        if self.is_in_arithmetic_expansion() {
            return NewlineContext::ArithmeticExpansion;
        }
        if self.posix_lexical.is_in_parameter_expansion() {
            return NewlineContext::ParameterExpansion;
        }
        if !self.is_plain_script_text() {
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
    pub(super) fn start_pending_heredoc_after(
        &mut self,
        context: NewlineContext,
    ) -> Result<(), IrGenError> {
        let boundary = HeredocBoundary::new(self.posix_lexical.current_frame_id(), context);
        self.posix_lexical
            .activate_pending_heredoc(boundary)
            .map_err(|reason| invalid_heredoc_boundary_error(self.template.to_owned(), reason))
    }

    /// Return whether the cursor sits in plain, executable script text.
    ///
    /// Heredoc declarations are recognized only here: quoted text and backtick
    /// bodies belong to the surrounding shell region, so a `<<` there is not
    /// modelled. Comment detection is wider: an open backtick body is its own
    /// command region, so `starts_comment` also accepts unquoted body text.
    pub(super) fn is_plain_script_text(&self) -> bool {
        self.active_quote_context() == ShellQuoteContext::Unquoted && self.backtick_region.is_none()
    }
}
