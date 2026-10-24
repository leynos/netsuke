//! Track assignment context used to classify PowerShell expression hashes.

use super::PowerShellLexicalState;
use crate::ir::cmd_interpolate::substitution::powershell::tokens::PowerShellSourceCharacter;

impl PowerShellLexicalState {
    /// Track assignment context and typographic quotes as the source advances.
    pub(in crate::ir::cmd_interpolate::substitution) fn note_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) {
        self.has_typographic_quote |= character.is_typographic_quote();
        match character.value() {
            ';' | '\n' | '\r' | '|' | '\'' | '"' => {
                self.reset_variable_assignment_context();
            }
            '=' => {
                self.has_variable_assignment_since_boundary =
                    character.assignment_targets_variable();
            }
            _ => {}
        }
    }

    /// Clear assignment context at the delimiters used by the comment lookaround.
    pub(super) const fn reset_variable_assignment_context(&mut self) {
        self.has_variable_assignment_since_boundary = false;
    }

    /// Report whether the nearest assignment before the current character targets a variable.
    pub(in crate::ir::cmd_interpolate::substitution) const fn has_variable_assignment_since_boundary(
        &self,
    ) -> bool {
        self.has_variable_assignment_since_boundary
    }
}
