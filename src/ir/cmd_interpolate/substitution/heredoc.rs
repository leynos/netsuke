//! Recognize POSIX heredoc declarations and comments in recipe text.
//!
//! A declaration is copied with its delimiter, so quote removal and any
//! rejection are decided once, before marker lowering resumes. The body that
//! follows stays inert until its terminator, which keeps inserted paths out of
//! text the shell expands. The helper stays separate from the traversal so each
//! module keeps one responsibility.

use super::super::{
    IrGenError, invalid_command_error, invalid_heredoc_delimiter_error,
    posix_lexical::{EscapeBeforeMarker, HeredocDeclaration, PosixCharacter, PosixLexicalState},
};
use super::SubstitutionTraversal;

impl SubstitutionTraversal<'_, '_> {
    /// Preserve unquoted POSIX heredoc declarations and comments.
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
        if !self.matches_unquoted_quote_context() || self.is_in_arithmetic_expansion() {
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
            HeredocDeclaration::NotADeclaration => {}
            HeredocDeclaration::Copied(next) => return Ok(Some(next)),
            HeredocDeclaration::Rejected(reason) => {
                return Err(invalid_heredoc_delimiter_error(
                    self.template.to_owned(),
                    reason,
                ));
            }
        }
        if PosixLexicalState::starts_comment(self.chars, pos, ch) {
            self.posix_lexical.begin_comment();
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        Ok(None)
    }
}
