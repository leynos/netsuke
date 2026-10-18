//! Track raw native-command arguments after PowerShell's `--%` token.

use super::super::super::{IrGenError, find_substitution, invalid_command_error};
use super::super::SubstitutionTraversal;
use super::lexical::PowerShellSourceCharacter;

/// Retain raw argument double-quote state until PowerShell resumes parsing.
pub(in crate::ir::cmd_interpolate::substitution) struct PowerShellStopParsingState {
    /// Record whether the traversal is inside the raw native-command region.
    is_active: bool,
    /// Retain an ASCII double quote delimiting a literal pipe in the raw region.
    quote: Option<char>,
}

impl PowerShellStopParsingState {
    /// Initialize raw native-command tracking for one traversal.
    pub(in crate::ir::cmd_interpolate::substitution) const fn new() -> Self {
        Self {
            is_active: false,
            quote: None,
        }
    }

    /// Begin a native-command region that bypasses normal PowerShell parsing.
    pub(super) const fn begin(&mut self) {
        self.is_active = true;
        self.quote = None;
    }

    /// Report whether raw stop-parsing text is currently being traversed.
    pub(in crate::ir::cmd_interpolate::substitution) const fn is_active(&self) -> bool {
        self.is_active
    }

    /// End raw argument parsing after copying its line or pipeline terminator.
    const fn end(&mut self) {
        self.is_active = false;
        self.quote = None;
    }

    /// Copy one raw argument character and retain quote-aware boundary state.
    fn append_character(
        &mut self,
        output: &mut String,
        character: PowerShellSourceCharacter,
    ) -> usize {
        let pos = character.position;
        let ch = character.value;
        output.push(ch);
        if matches!(ch, '\r' | '\n') {
            self.end();
            return pos + 1;
        }
        self.update_quote_state(ch);
        if self.quote.is_none() && ch == '|' {
            self.end();
        }
        pos + 1
    }

    /// Update the ASCII double-quote state used to recognize raw pipelines.
    const fn update_quote_state(&mut self, character: char) {
        if character == '"' {
            self.quote = if self.quote.is_some() {
                None
            } else {
                Some('"')
            };
        }
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Preserve raw native-command text while rejecting recipe markers.
    ///
    /// Only ASCII double quotes are modelled when finding raw-region pipelines.
    /// A single quote or backtick makes later marker sites fail closed because
    /// the traversal does not model how that syntax interacts with resumed
    /// PowerShell parsing. Typographic quotes use the same fail-closed policy.
    ///
    /// # Errors
    ///
    /// Returns `InvalidCommand` when a recipe marker occurs in raw arguments.
    pub(super) fn append_stop_parsing_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) -> Result<usize, IrGenError> {
        if find_substitution(self.chars, character.position).is_some() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        if matches!(character.value, '\'' | '`') {
            self.power_shell_lexical.note_unmodelled_syntax();
        }
        self.power_shell_lexical.note_character(character.value);
        Ok(self
            .power_shell_stop_parsing
            .append_character(&mut self.output, character))
    }
}
