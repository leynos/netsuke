//! Track PowerShell block comments so their contents stay lexically inert.
//!
//! A block comment starts at `<#` and ends at the first `#>`; PowerShell does
//! not nest them, so a `<#` inside the body is ordinary comment text. Modelling
//! that exactly keeps quotes, parentheses, and markers inside the comment from
//! perturbing the surrounding script, which is what lets a shared command-list
//! entry close its comment and hand a neutral boundary to its successor.

use super::super::super::{
    IrGenError, RejectionReason, SubstitutionTraversal, find_substitution, invalid_command_error,
};
use super::super::tokens::PowerShellSourceCharacter;
use super::PowerShellLexicalState;

impl PowerShellLexicalState {
    /// Report whether the traversal is inside a block comment.
    pub(super) const fn is_in_block_comment(&self) -> bool {
        self.block_comment_depth > 0
    }

    /// Record that a block comment is open.
    ///
    /// The field stays a count so `unfinished_region` reads as a depth check,
    /// but PowerShell comments do not nest, so this never exceeds one.
    const fn open_block_comment(&mut self) {
        self.block_comment_depth = 1;
    }

    /// Record that the block comment's terminating `#>` was read.
    const fn close_block_comment(&mut self, position: usize) {
        self.block_comment_depth = 0;
        self.block_comment_end_position = Some(position);
    }

    /// Report whether `position` immediately follows a recognised `#>`.
    pub(super) const fn follows_block_comment_end(&self, position: usize) -> bool {
        matches!(self.block_comment_end_position, Some(end) if end == position)
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Report whether PowerShell can begin a token at this character.
    ///
    /// A completed block comment counts as whitespace, so the character after
    /// `#>` begins a token even though the raw preceding character is `>`.
    pub(super) fn follows_power_shell_token_boundary(
        &self,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        character.is_token_boundary()
            || self
                .power_shell_lexical
                .follows_block_comment_end(character.position())
    }

    /// Open a block comment at an unquoted PowerShell token boundary.
    ///
    /// Callers reach this check only outside here-strings, line comments, and
    /// `--%` regions, so an unquoted site is the whole remaining condition.
    /// PowerShell recognizes `<#` only where a token can begin; inside an
    /// argument such as `prefix<#" #>` it stays part of the argument token and
    /// the quotes around it still open an expandable string. A mid-token
    /// opener, or an opener inside a mid-token hash that may itself begin a
    /// line comment, therefore fails closed instead of opening a region the
    /// interpreter may not share.
    pub(super) fn open_unquoted_block_comment(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Option<usize> {
        if !self.matches_unquoted_quote_context() || !character.starts_block_comment() {
            return None;
        }
        if self.power_shell_lexical.is_in_ambiguous_comment()
            || !self.follows_power_shell_token_boundary(character)
        {
            self.power_shell_lexical.note_unmodelled_syntax();
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
            return Err(invalid_command_error(
                self.template.to_owned(),
                RejectionReason::MarkerInProtectedRegion,
            ));
        }
        if character.ends_block_comment() {
            self.power_shell_lexical.close_block_comment(pos + 2);
            self.output.push('#');
            self.output.push('>');
            return Ok(pos + 2);
        }
        // PowerShell does not nest block comments, so an inner `<#` is ordinary
        // comment text rather than a deeper region.
        self.output.push(character.value());
        Ok(pos + 1)
    }
}
