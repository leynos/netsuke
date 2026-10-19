//! Track comment syntax that can desynchronize PowerShell marker scanning.

use super::super::super::{IrGenError, SubstitutionTraversal};
use super::super::tokens::{
    comment_character_changes_scanner_state, hash_follows_ambiguous_expression_boundary,
    is_power_shell_comment_boundary, starts_block_comment,
};
use super::{PowerShellCommentState, PowerShellLexicalState, PowerShellSourceCharacter};

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

    /// Track scanner-sensitive syntax inside a confirmed line comment.
    const fn note_unquoted_comment_character(
        &mut self,
        changes_scanner_state: bool,
        is_line_break: bool,
    ) {
        self.has_unmodelled_syntax |= changes_scanner_state;
        if is_line_break {
            self.comment_state = PowerShellCommentState::None;
        }
    }

    /// Track scanner-sensitive syntax after a hash with an uncertain boundary.
    pub(super) const fn note_ambiguous_comment_character(
        &mut self,
        changes_scanner_state: bool,
        is_line_break: bool,
    ) {
        self.has_unmodelled_syntax |= changes_scanner_state;
        if is_line_break {
            self.comment_state = PowerShellCommentState::None;
        }
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Report whether an unquoted hash begins a PowerShell comment token.
    pub(super) fn is_unquoted_comment_start(&self, character: PowerShellSourceCharacter) -> bool {
        character.value == '#'
            && self.matches_unquoted_quote_context()
            && is_power_shell_comment_boundary(self.chars, character.position)
    }

    /// Identify a mid-token hash whose tokenizer boundary may be ambiguous.
    pub(super) fn is_ambiguous_unquoted_comment_start(
        &self,
        character: PowerShellSourceCharacter,
    ) -> bool {
        character.value == '#'
            && self.matches_unquoted_quote_context()
            && hash_follows_ambiguous_expression_boundary(self.chars, character.position)
    }

    /// Track scanner-sensitive syntax while a mid-token hash may be a comment.
    pub(super) fn note_ambiguous_comment_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) {
        let changes_scanner_state = comment_character_changes_scanner_state(
            self.chars,
            character.position,
            character.value,
            self.is_in_command_substitution(),
        );
        self.power_shell_lexical.note_ambiguous_comment_character(
            changes_scanner_state,
            matches!(character.value, '\r' | '\n'),
        );
    }

    /// Track ambiguous syntax and preserve a PowerShell comment character.
    pub(super) fn append_unquoted_comment_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) -> Result<usize, IrGenError> {
        let changes_scanner_state = comment_character_changes_scanner_state(
            self.chars,
            character.position,
            character.value,
            self.is_in_command_substitution(),
        );
        self.power_shell_lexical.note_unquoted_comment_character(
            changes_scanner_state,
            matches!(character.value, '\r' | '\n'),
        );
        self.append_marker_for_context(
            character.position,
            character.value,
            Self::power_shell_marker_protection(self),
        )
    }

    /// Remember a block comment that the partial scanner cannot parse safely.
    pub(super) fn note_unparsed_block_comment_opener(
        &mut self,
        character: PowerShellSourceCharacter,
    ) {
        if self.matches_unquoted_quote_context()
            && starts_block_comment(self.chars, character.position)
        {
            self.power_shell_lexical.note_unmodelled_syntax();
        }
    }
}
