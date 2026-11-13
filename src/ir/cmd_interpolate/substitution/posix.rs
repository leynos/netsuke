//! Resolve POSIX shell characters that decide whether a marker may be lowered.
//!
//! Heredoc declarations, comments, and backslash escapes are inert or
//! rejected before the shared traversal reaches marker handling, so they are
//! resolved first. Keeping them in a sibling module holds the parent within
//! the repository's module-size cap.

use super::super::posix_lexical::{
    EscapeBeforeMarker, PosixCharacter, PosixLexicalState, backslash_escapes_marker,
};
use super::{IrGenError, MarkerProtection, SubstitutionTraversal, invalid_command_error};

impl SubstitutionTraversal<'_, '_> {
    /// Preserve POSIX shell syntax while lowering context-safe recipe markers.
    pub(super) fn append_posix_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<usize, IrGenError> {
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
            self.in_backticks ^= true;
            self.output.push(ch);
            return Ok(pos + 1);
        }
        let next = self.append_contextual_character(pos, ch, Self::posix_marker_protection)?;
        if ch == '\n' {
            self.posix_lexical.begin_pending_heredoc_after_newline(next);
        }
        Ok(next)
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
    /// # Errors
    ///
    /// Returns an invalid-command error when a heredoc delimiter would lower a
    /// marker directly after an active escape.
    fn append_unquoted_posix_lexical_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if !self.matches_unquoted_quote_context() {
            return Ok(None);
        }
        let mut character = PosixCharacter {
            chars: self.chars,
            pos,
            ch,
            output: &mut self.output,
        };
        let declaration_end = self
            .posix_lexical
            .append_heredoc_declaration(&mut character, self.bindings)
            .map_err(|EscapeBeforeMarker| invalid_command_error(self.template.to_owned()))?;
        if let Some(next) = declaration_end {
            return Ok(Some(next));
        }
        if PosixLexicalState::starts_comment(self.chars, pos, ch, self.closed_command_substitution)
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
