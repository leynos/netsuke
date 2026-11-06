//! Recognise POSIX heredoc declarations and comments in plain script text.
//!
//! A declaration is copied with its delimiter, so quote removal and any
//! rejection are decided once, before marker lowering resumes. The body that
//! follows stays inert until its terminator, which keeps inserted paths out of
//! text the shell expands. The helpers stay separate from the traversal so each
//! module keeps one responsibility.

use super::super::{
    IrGenError, invalid_command_error, invalid_heredoc_delimiter_error,
    posix_lexical::{EscapeBeforeMarker, HeredocDeclaration, PosixCharacter, PosixLexicalState},
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
        if !self.is_plain_script_text() || self.is_in_arithmetic_expansion() {
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
            .map_err(|EscapeBeforeMarker| invalid_command_error(self.template.to_owned()))?;
        match declaration {
            HeredocDeclaration::NotADeclaration => Ok(None),
            HeredocDeclaration::Copied(next) => Ok(Some(next)),
            HeredocDeclaration::Rejected(reason) => Err(invalid_heredoc_delimiter_error(
                self.template.to_owned(),
                reason,
            )),
        }
    }

    /// Return whether the cursor sits in plain, executable script text.
    ///
    /// Comments and heredoc declarations are recognized only here: quoted text
    /// and backtick bodies belong to the surrounding shell region.
    fn is_plain_script_text(&self) -> bool {
        self.active_quote_context() == ShellQuoteContext::Unquoted
            && self.backtick_outer_context.is_none()
    }

    /// Return whether `ch` begins a POSIX shell comment in ordinary text.
    pub(super) fn starts_comment(&self, pos: usize, ch: char) -> bool {
        self.is_plain_script_text() && PosixLexicalState::starts_comment(self.chars, pos, ch)
    }
}
