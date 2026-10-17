//! Track raw native-command arguments after PowerShell's `--%` token.

use super::super::super::{IrGenError, find_substitution, invalid_command_error};
use super::super::SubstitutionTraversal;
use super::lexical::PowerShellSourceCharacter;

/// Retain raw argument quote state until PowerShell resumes parsing.
pub(in crate::ir::cmd_interpolate::substitution) struct PowerShellStopParsingState {
    /// Record whether the traversal is inside the raw native-command region.
    is_active: bool,
    /// Retain a quote delimiting a literal pipe within the raw region.
    quote: Option<char>,
    /// Preserve a quote escaped by a PowerShell backtick in the raw region.
    is_escaped: bool,
}

impl PowerShellStopParsingState {
    /// Initialise raw native-command tracking for one traversal.
    pub(in crate::ir::cmd_interpolate::substitution) const fn new() -> Self {
        Self {
            is_active: false,
            quote: None,
            is_escaped: false,
        }
    }

    /// Begin a native-command region that bypasses normal PowerShell parsing.
    pub(super) const fn begin(&mut self) {
        self.is_active = true;
        self.quote = None;
        self.is_escaped = false;
    }

    /// Report whether raw stop-parsing text is currently being traversed.
    pub(in crate::ir::cmd_interpolate::substitution) const fn is_active(&self) -> bool {
        self.is_active
    }

    /// End raw argument parsing after copying its line or pipeline terminator.
    const fn end(&mut self) {
        self.is_active = false;
        self.quote = None;
        self.is_escaped = false;
    }

    /// Copy one raw argument character and retain quote-aware boundary state.
    fn append_character(
        &mut self,
        chars: &[char],
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
        if let Some(next_position) = self.update_quote_state(chars, output, character) {
            return next_position;
        }
        if self.quote.is_none() && ch == '|' {
            self.end();
        }
        pos + 1
    }

    /// Update quote and escape state for one raw native-command argument.
    fn update_quote_state(
        &mut self,
        chars: &[char],
        output: &mut String,
        character: PowerShellSourceCharacter,
    ) -> Option<usize> {
        let pos = character.position;
        let ch = character.value;
        if self.is_escaped {
            self.is_escaped = false;
            return None;
        }
        match (self.quote, ch) {
            (Some('"'), '`') => self.is_escaped = true,
            (Some('\''), '\'') if chars.get(pos + 1) == Some(&'\'') => {
                output.push('\'');
                return Some(pos + 2);
            }
            (Some(quote), matched_character) if quote == matched_character => self.quote = None,
            (None, '\'' | '"') => self.quote = Some(ch),
            _ => {}
        }
        None
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Preserve raw native-command text while rejecting recipe markers.
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
        Ok(self
            .power_shell_stop_parsing
            .append_character(self.chars, &mut self.output, character))
    }
}
