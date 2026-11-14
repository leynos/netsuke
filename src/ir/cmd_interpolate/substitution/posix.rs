//! Resolve POSIX shell characters that decide whether a marker may be lowered.
//!
//! Heredoc declarations, comments, and backslash escapes are inert or
//! rejected before the shared traversal reaches marker handling, so they are
//! resolved first. Keeping them in a sibling module holds the parent within
//! the repository's module-size cap.

use super::super::invalid_heredoc_delimiter_error;
use super::super::posix_lexical::{
    EscapeBeforeMarker, HeredocDeclaration, PosixCharacter, SourceChar, backslash_escapes_marker,
};
use super::{IrGenError, MarkerProtection, SubstitutionTraversal, invalid_command_error};

impl SubstitutionTraversal<'_, '_> {
    /// Preserve POSIX shell syntax while lowering context-safe recipe markers.
    pub(super) fn append_posix_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<usize, IrGenError> {
        if let Some(next) = self.close_backtick_comment(pos, ch) {
            return Ok(next);
        }
        if let Some(next) = self.copy_inert_character(pos, ch) {
            return Ok(next);
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
            self.posix_lexical.begin_pending_heredoc_after_newline();
        }
        Ok(next)
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
    /// Comment text and heredoc bodies are copied without inspection, so the
    /// traversal is the only part of the pass that can tell the tracker which
    /// newline ended a comment.
    fn copy_inert_character(&mut self, pos: usize, ch: char) -> Option<usize> {
        let ends_comment = self.posix_lexical.in_comment() && ch == '\n';
        let mut character = PosixCharacter {
            chars: self.chars,
            pos,
            ch,
            output: &mut self.output,
        };
        if !self.posix_lexical.append_inert_character(&mut character) {
            return None;
        }
        if ends_comment {
            self.end_case_comment();
        }
        Some(pos + 1)
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
            .map_err(|EscapeBeforeMarker| invalid_command_error(self.template.to_owned()))?;
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
            return Err(invalid_command_error(self.template.to_owned()));
        }
        let Some(next) = self.chars.get(pos + 1).copied() else {
            return Ok(None);
        };
        self.output.push(ch);
        self.output.push(next);
        Ok(Some(pos + 2))
    }
}
