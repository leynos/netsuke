//! Track PowerShell block comments so their contents stay lexically inert.
//!
//! PowerShell block comments nest, and a `#>` inside one only closes the
//! innermost level. Modelling the depth exactly keeps quotes, parentheses, and
//! markers inside the comment from perturbing the surrounding script, which is
//! what lets a shared command-list entry close its comment and hand a neutral
//! boundary to its successor.

use super::super::super::{
    IrGenError, SubstitutionTraversal, find_substitution, invalid_command_error,
};
use super::super::tokens::PowerShellSourceCharacter;
use super::PowerShellLexicalState;

impl PowerShellLexicalState {
    /// Report whether the traversal is inside a block comment.
    pub(super) const fn is_in_block_comment(&self) -> bool {
        self.block_comment_depth > 0
    }

    /// Record one block-comment nesting level.
    const fn open_block_comment(&mut self) {
        self.block_comment_depth += 1;
    }

    /// Release one block-comment nesting level.
    const fn close_block_comment(&mut self) {
        self.block_comment_depth = self.block_comment_depth.saturating_sub(1);
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Open a block comment at an unquoted PowerShell site.
    ///
    /// Callers reach this check only outside here-strings, line comments, and
    /// `--%` regions, so an unquoted site is the whole remaining condition.
    pub(super) fn open_unquoted_block_comment(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Option<usize> {
        if !self.matches_unquoted_quote_context() || !character.starts_block_comment() {
            return None;
        }
        self.power_shell_lexical.open_block_comment();
        self.output.push('<');
        self.output.push('#');
        Some(character.position() + 2)
    }

    /// Preserve block-comment text as inert script, rejecting recipe markers.
    ///
    /// A marker inside a comment would survive as the internal placeholder
    /// token rather than a path, so it is rejected like a marker inside a
    /// here-string body.
    pub(super) fn append_block_comment_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Result<usize, IrGenError> {
        let pos = character.position();
        if find_substitution(self.chars, pos).is_some() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        if character.ends_block_comment() {
            self.power_shell_lexical.close_block_comment();
            self.output.push('#');
            self.output.push('>');
            return Ok(pos + 2);
        }
        if character.starts_block_comment() {
            // PowerShell nests block comments, so an inner opener only deepens
            // the region the outer terminator must eventually unwind.
            self.power_shell_lexical.open_block_comment();
            self.output.push('<');
            self.output.push('#');
            return Ok(pos + 2);
        }
        self.output.push(character.value());
        Ok(pos + 1)
    }
}
