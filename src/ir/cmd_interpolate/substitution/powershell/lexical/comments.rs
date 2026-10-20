//! Track comment syntax that can desynchronize PowerShell marker scanning.

use super::super::super::{IrGenError, SubstitutionTraversal};
use super::super::tokens::{
    PowerShellCommandSubstitutionContext, PowerShellSourceCharacter,
    comment_character_changes_scanner_state,
};
use super::{PowerShellCommentState, PowerShellLexicalState};

impl PowerShellLexicalState {
    /// Begin tracking a line comment at a PowerShell comment boundary.
    pub(super) const fn begin_unquoted_comment(&mut self) {
        self.comment_state = PowerShellCommentState::Unquoted;
    }

    /// Begin tracking scanner-sensitive characters after a mid-token hash.
    pub(super) const fn begin_ambiguous_comment(&mut self) {
        self.comment_state = PowerShellCommentState::Ambiguous;
    }

    /// Report whether the current character belongs to an unquoted comment.
    pub(super) const fn is_in_unquoted_comment(&self) -> bool {
        matches!(self.comment_state, PowerShellCommentState::Unquoted)
    }

    /// Report whether a mid-token hash may hide scanner-sensitive characters.
    pub(super) const fn is_in_ambiguous_comment(&self) -> bool {
        matches!(self.comment_state, PowerShellCommentState::Ambiguous)
    }

    /// Accumulate scanner-sensitive syntax and end comment mode at line breaks.
    pub(super) fn note_comment_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
        context: PowerShellCommandSubstitutionContext,
    ) {
        self.has_unmodelled_syntax |= comment_character_changes_scanner_state(character, context);
        if character.is_line_break() {
            self.comment_state = PowerShellCommentState::None;
            self.reset_variable_assignment_context();
        }
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Report whether an unquoted hash begins a PowerShell comment token.
    pub(super) fn is_unquoted_comment_start(
        &self,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        character.value() == '#'
            && self.matches_unquoted_quote_context()
            && character.is_token_boundary()
    }

    /// Identify a mid-token hash after expression syntax that may start a comment.
    pub(super) fn is_ambiguous_unquoted_comment_start(
        &self,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        character.value() == '#'
            && self.matches_unquoted_quote_context()
            && character.hash_follows_ambiguous_expression_boundary(
                self.power_shell_lexical
                    .has_variable_assignment_since_boundary(),
            )
    }

    /// Track scanner-sensitive syntax while a mid-token hash may be a comment.
    pub(super) fn note_ambiguous_comment_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) {
        self.note_comment_character(character);
    }

    /// Track ambiguous syntax and preserve a PowerShell comment character.
    pub(super) fn append_unquoted_comment_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Result<usize, IrGenError> {
        self.note_comment_character(character);
        self.append_marker_for_context(
            character.position(),
            character.value(),
            Self::power_shell_marker_protection(self, character.position()),
        )
    }

    /// Record comment-sensitive syntax without merging its two traversal paths.
    fn note_comment_character(&mut self, character: PowerShellSourceCharacter<'_>) {
        let context = if self.is_in_command_substitution() {
            PowerShellCommandSubstitutionContext::Inside
        } else {
            PowerShellCommandSubstitutionContext::Outside
        };
        self.power_shell_lexical
            .note_comment_character(character, context);
    }

    /// Remember a block comment that the partial scanner cannot parse safely.
    pub(super) fn note_unparsed_block_comment_opener(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) {
        if self.matches_unquoted_quote_context() && character.starts_block_comment() {
            self.power_shell_lexical.note_unmodelled_syntax();
        }
    }
}
