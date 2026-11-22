//! Classify backslashes in script text and inside backtick bodies.
//!
//! A backtick body is parsed twice: the delimiter scan removes the backslash
//! of a `\` before a dollar sign, a backtick, or a backslash, and the body's
//! own parse then reads the remaining text. These helpers keep those two
//! escapes distinct so a surviving backslash still protects the character that
//! follows it, whether that character is a frame delimiter, a quote, or a
//! marker.

use super::super::{
    IrGenError, RejectionReason, find_script_substitution, invalid_command_error,
    posix_lexical::backslash_escapes_marker,
};
use super::backticks::{BacktickRegion, escapes_backtick_body_character};
use super::{ScriptSubstitutionTraversal, ShellQuoteContext};

impl ScriptSubstitutionTraversal<'_, '_> {
    /// Copy a backtick-body escape pair and record the backslash it leaves.
    ///
    /// The delimiter scan removes the backslash of a `\` before a dollar sign,
    /// a backtick, or a backslash; the body's own parse then reads the
    /// remaining text. A `\\` pair therefore leaves one backslash for that
    /// parse, which escapes whichever character reaches it next. Recording it
    /// here keeps the two escapes distinct: the pair is consumed for the
    /// delimiter scan, while the survivor is still active.
    ///
    /// The active quoting decides whether the survivor is active at all. In
    /// the body's single quotes a backslash has no meaning, so the character
    /// after the pair is ordinary data and must still toggle quotes.
    pub(super) fn append_backtick_body_escape(&mut self, pos: usize, ch: char) -> Option<usize> {
        if self.backtick_region.is_none() || ch != '\\' {
            return None;
        }
        if !escapes_backtick_body_character(self.chars, pos) {
            return None;
        }
        let next = *self.chars.get(pos + 1)?;
        self.output.push(ch);
        self.output.push(next);
        if next != '\\' || self.active_quote_context() == ShellQuoteContext::SingleQuoted {
            return Some(pos + 2);
        }
        if let Some(region) = self.backtick_region.as_mut() {
            region.mark_residual_escape();
        }
        Some(pos + 2)
    }

    /// Copy the character that the body's own parse reads as escaped.
    ///
    /// The backslash a `\\` pair leaves is active in that parse, so the next
    /// character cannot open a quote, close a `$()` frame, or start a
    /// substitution. A marker there still fails closed: copying it without the
    /// marker scan would leave the internal token in the lowered script.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the escaped character begins a
    /// marker token.
    pub(super) fn append_residual_escape_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if !self.take_residual_escape() {
            return Ok(None);
        }
        // An unescaped backtick is the closer the delimiter scan found, so the
        // region ends here; a dangling backslash is then the shell's own
        // syntax problem rather than a reason to copy the closer.
        if ch == '`' {
            return Ok(None);
        }
        if ch == '\\' && escapes_backtick_body_character(self.chars, pos) {
            // The delimiter scan removes this backslash too, and the residual
            // escapes the character it protected.
            if let Some(next) = self.chars.get(pos + 1) {
                self.output.push(ch);
                self.output.push(*next);
                return Ok(Some(pos + 2));
            }
        }
        if find_script_substitution(self.chars, pos).is_some() {
            return Err(invalid_command_error(
                self.template.to_owned(),
                RejectionReason::MarkerInProtectedRegion,
            ));
        }
        self.output.push(ch);
        Ok(Some(pos + 1))
    }

    /// Report whether a backslash begins an escape of the following character.
    ///
    /// Backtick-body escape pairs are consumed before this check, and an open
    /// body's remaining escapes follow its own quoting at the owner depth.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the escape would modify a marker
    /// lowered immediately after it.
    pub(super) fn starts_escape(&self, pos: usize, ch: char) -> Result<bool, IrGenError> {
        if ch != '\\' {
            return Ok(false);
        }
        if let Some(escapes) = self.starts_body_escape(pos)? {
            return Ok(escapes);
        }
        if self.active_quote_context() == ShellQuoteContext::SingleQuoted {
            return Ok(false);
        }
        self.reject_marker_escape(pos)?;
        Ok(true)
    }

    /// Classify a backslash under an open backtick body.
    ///
    /// Returns `None` when the body's rules do not decide the escape and the
    /// innermost region's own quoting must. Escape pairs that reach a `$()`
    /// frame opened in the body were already consumed with their residual
    /// backslash, so only the body's own quoting at the owner depth decides
    /// here.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the escape would modify a marker
    /// lowered immediately after it.
    fn starts_body_escape(&self, pos: usize) -> Result<Option<bool>, IrGenError> {
        let Some(region) = self.backtick_region else {
            return Ok(None);
        };
        if self.command_substitutions.len() != region.owner_depth {
            return Ok(None);
        }
        let escapes = region.escapes_body_character(self.chars, pos);
        if escapes {
            self.reject_marker_escape(pos)?;
        }
        Ok(Some(escapes))
    }

    /// Take and clear the open backtick body's pending residual backslash.
    fn take_residual_escape(&mut self) -> bool {
        self.backtick_region
            .as_mut()
            .is_some_and(BacktickRegion::take_residual_escape)
    }

    /// Reject an escape that would consume a marker's first character.
    ///
    /// The marker scan starts at that character, so letting the escape win
    /// would copy the rest of the internal token into the lowered script
    /// instead of failing closed.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the following character begins a
    /// marker token.
    fn reject_marker_escape(&self, pos: usize) -> Result<(), IrGenError> {
        if backslash_escapes_marker(self.chars, pos, false, None) {
            return Err(invalid_command_error(
                self.template.to_owned(),
                RejectionReason::EscapeBeforeMarker,
            ));
        }
        Ok(())
    }
}
