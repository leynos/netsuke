//! Track PowerShell lexical state that can make marker substitution unsafe.

mod comments;

use super::super::super::{IrGenError, QuoteContext, find_substitution, invalid_command_error};
use super::super::{MarkerProtection, SubstitutionTraversal};
use super::tokens::{
    here_string_opener, is_power_shell_here_string_boundary, is_typographic_quote,
    starts_stop_parsing_token,
};
use crate::shell_word::{is_power_shell_double_quote, is_power_shell_single_quote};

/// Group a PowerShell source offset with the character being traversed.
///
/// Keep this carrier local to the PowerShell traversal; it does not model a
/// general shell character or replace the POSIX scanner's character type.
#[derive(Clone, Copy)]
pub(in crate::ir::cmd_interpolate::substitution) struct PowerShellSourceCharacter {
    /// Identify the source position used for lexical lookahead.
    pub(in crate::ir::cmd_interpolate::substitution) position: usize,
    /// Preserve the character at that position.
    pub(in crate::ir::cmd_interpolate::substitution) value: char,
}

/// Track the comment mode that affects PowerShell marker scanning.
enum PowerShellCommentState {
    /// No line comment can change the scanner state.
    None,
    /// A comment began at a confirmed token boundary.
    Unquoted,
    /// A mid-token hash may be interpreted as a comment by PowerShell.
    Ambiguous,
}

/// Track PowerShell quotes, here-strings, comments, and native-command regions.
pub(in crate::ir::cmd_interpolate::substitution) struct PowerShellLexicalState {
    /// Retain the opener quote until its matching closing marker.
    here_string_quote: Option<char>,
    /// Fail closed after a typographic quote whose pairing is not modelled.
    has_typographic_quote: bool,
    /// Track comments so scanner-sensitive contents can fail closed.
    comment_state: PowerShellCommentState,
    /// Fail closed after PowerShell syntax the partial scanner does not model.
    has_unmodelled_syntax: bool,
}

/// Classify the current character while a here-string may be active.
enum HereStringCharacter {
    /// No here-string is active, so normal PowerShell parsing can continue.
    Outside,
    /// Preserve one character from the current here-string body.
    Copied,
    /// Preserve a closing marker and resume normal PowerShell parsing.
    Closed(char),
    /// Refuse a recipe marker inside the here-string body.
    Placeholder,
}

impl PowerShellLexicalState {
    /// Initialize PowerShell here-string tracking for one traversal.
    pub(in crate::ir::cmd_interpolate::substitution) const fn new() -> Self {
        Self {
            here_string_quote: None,
            has_typographic_quote: false,
            comment_state: PowerShellCommentState::None,
            has_unmodelled_syntax: false,
        }
    }

    /// Remember typographic quote characters so later markers fail closed.
    pub(in crate::ir::cmd_interpolate::substitution) const fn note_character(&mut self, ch: char) {
        self.has_typographic_quote |= is_typographic_quote(ch);
    }

    /// Report whether a typographic quote requires conservative marker rejection.
    pub(in crate::ir::cmd_interpolate::substitution) const fn has_typographic_quote(&self) -> bool {
        self.has_typographic_quote
    }

    /// Fail closed after syntax the partial scanner cannot safely parse.
    pub(in crate::ir::cmd_interpolate::substitution) const fn note_unmodelled_syntax(&mut self) {
        self.has_unmodelled_syntax = true;
    }

    /// Report whether unmodelled syntax requires conservative marker rejection.
    pub(in crate::ir::cmd_interpolate::substitution) const fn has_unmodelled_syntax(&self) -> bool {
        self.has_unmodelled_syntax
    }

    /// Classify the current character while tracking an active here-string body.
    fn classify_here_string_character(
        &mut self,
        chars: &[char],
        pos: usize,
        has_marker: bool,
    ) -> HereStringCharacter {
        let Some(quote) = self.here_string_quote else {
            return HereStringCharacter::Outside;
        };
        if is_here_string_terminator(chars, pos, quote) {
            self.here_string_quote = None;
            return HereStringCharacter::Closed(quote);
        }
        if is_typographic_here_string_terminator(chars, pos, quote) {
            self.note_unmodelled_syntax();
        }
        if has_marker {
            return HereStringCharacter::Placeholder;
        }
        // PowerShell treats quotes inside a here-string as literal body text.
        HereStringCharacter::Copied
    }

    /// Begin a here-string when an unquoted opener starts at `pos`.
    fn try_open_here_string(
        &mut self,
        chars: &[char],
        pos: usize,
        is_unquoted_site: bool,
    ) -> Option<char> {
        if !is_unquoted_site {
            return None;
        }
        let quote = here_string_opener(chars, pos)?;
        self.here_string_quote = Some(quote);
        Some(quote)
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Preserve PowerShell text while lowering only manifest-owned markers.
    pub(in crate::ir::cmd_interpolate::substitution) fn append_power_shell_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) -> Result<usize, IrGenError> {
        let pos = character.position;
        let ch = character.value;
        if self.power_shell_stop_parsing.is_active() {
            return self.append_stop_parsing_character(character);
        }
        if let Some(next) = self.append_here_string_character(character)? {
            return Ok(next);
        }
        if self.power_shell_lexical.is_in_unquoted_comment() {
            return self.append_unquoted_comment_character(character);
        }
        if self.power_shell_lexical.is_in_ambiguous_comment() {
            self.note_ambiguous_comment_character(character);
        }
        self.note_unparsed_block_comment_opener(character);
        if let Some(next) = self.open_here_string(character) {
            return Ok(next);
        }
        self.power_shell_lexical.note_character(ch);
        // Preserve hashes inside argument tokens while distinguishing
        // confirmed comment boundaries from ambiguous expression syntax.
        if self.is_unquoted_comment_start(character) {
            self.power_shell_lexical.begin_unquoted_comment();
        } else if self.is_ambiguous_unquoted_comment_start(character) {
            self.power_shell_lexical.begin_ambiguous_comment();
        }
        if let Some(next) = self.append_power_shell_escaped_character(character)? {
            return Ok(next);
        }
        if self.append_power_shell_doubled_quote(character) {
            return Ok(pos + 2);
        }
        self.append_contextual_character(pos, ch, Self::power_shell_marker_protection)
    }

    /// Preserve or reject a character in an active PowerShell here-string.
    fn append_here_string_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) -> Result<Option<usize>, IrGenError> {
        let pos = character.position;
        match self.power_shell_lexical.classify_here_string_character(
            self.chars,
            pos,
            find_substitution(self.chars, pos).is_some(),
        ) {
            HereStringCharacter::Outside => Ok(None),
            HereStringCharacter::Copied => {
                self.output.push(character.value);
                Ok(Some(pos + 1))
            }
            HereStringCharacter::Closed(quote) => {
                self.output.push(quote);
                self.output.push('@');
                Ok(Some(pos + 2))
            }
            HereStringCharacter::Placeholder => {
                Err(invalid_command_error(self.template.to_owned()))
            }
        }
    }

    /// Open a here-string only at a safe unquoted PowerShell site.
    fn open_here_string(&mut self, character: PowerShellSourceCharacter) -> Option<usize> {
        self.note_unmodelled_power_shell_syntax(character);
        let is_unquoted_site = self.matches_unquoted_quote_context()
            && !self.in_backticks
            && !self.is_in_command_substitution();
        let quote = self.power_shell_lexical.try_open_here_string(
            self.chars,
            character.position,
            is_unquoted_site,
        )?;
        self.output.push('@');
        self.output.push(quote);
        Some(character.position + 2)
    }

    /// Identify a here-string opener embedded in a PowerShell token.
    fn is_mid_token_here_string_opener(&self, is_unquoted_site: bool, position: usize) -> bool {
        is_unquoted_site
            && here_string_opener(self.chars, position).is_some()
            && !is_power_shell_here_string_boundary(self.chars, position)
    }
    /// Fail closed when syntax can desynchronize the partial PowerShell scanner.
    fn note_unmodelled_power_shell_syntax(&mut self, character: PowerShellSourceCharacter) {
        let position = character.position;
        let here_string_in_command_substitution = self.is_in_command_substitution()
            && self.matches_unquoted_quote_context()
            && here_string_opener(self.chars, position).is_some();
        let braced_variable_name = character.value == '$'
            && self.chars.get(position + 1) == Some(&'{')
            && !self.matches_single_quote_context();
        let stop_parsing_token = !self.in_backticks
            && self.matches_unquoted_quote_context()
            && starts_stop_parsing_token(self.chars, position);
        if here_string_in_command_substitution || braced_variable_name {
            self.power_shell_lexical.note_unmodelled_syntax();
        }
        if stop_parsing_token {
            self.power_shell_stop_parsing.begin();
        }
    }

    /// Classify PowerShell quotes and command substitutions as protected regions.
    fn power_shell_marker_protection(&self) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            !matches!(self.active_quote_context(), QuoteContext::Unquoted)
                || self.power_shell_lexical.has_typographic_quote()
                || self.power_shell_lexical.has_unmodelled_syntax()
                || self.is_in_command_substitution(),
        )
    }

    /// Preserve a PowerShell backtick escape unless it would hide a recipe marker.
    fn append_power_shell_escaped_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) -> Result<Option<usize>, IrGenError> {
        let pos = character.position;
        let ch = character.value;
        if ch != '`' || self.matches_single_quote_context() {
            return Ok(None);
        }
        if find_substitution(self.chars, pos + 1).is_some() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        let Some(next) = self.chars.get(pos + 1) else {
            return Ok(None);
        };
        self.output.push(ch);
        self.output.push(*next);
        Ok(Some(pos + 2))
    }

    /// Preserve doubled quote characters inside a PowerShell string literal.
    fn append_power_shell_doubled_quote(&mut self, character: PowerShellSourceCharacter) -> bool {
        let pos = character.position;
        let ch = character.value;
        let Some(next) = self.chars.get(pos + 1).copied() else {
            return false;
        };
        let is_doubled_quote = matches!(
            (self.active_quote_context(), ch, next),
            (QuoteContext::Single(quote) | QuoteContext::Double(quote), current, following)
                if quote == current && quote == following
        );
        if is_doubled_quote {
            self.output.push(ch);
            self.output.push(next);
        }
        is_doubled_quote
    }
}

/// Report whether a here-string closes at the start of the current line.
fn is_here_string_terminator(chars: &[char], pos: usize, quote: char) -> bool {
    let is_line_start = pos == 0 || chars.get(pos - 1) == Some(&'\n');
    is_line_start && chars.get(pos) == Some(&quote) && chars.get(pos + 1) == Some(&'@')
}

/// Report whether a typographic quote from the opener's family may close the body.
fn is_typographic_here_string_terminator(chars: &[char], pos: usize, quote: char) -> bool {
    let is_line_start = pos == 0 || chars.get(pos - 1) == Some(&'\n');
    let Some(&character) = chars.get(pos) else {
        return false;
    };
    let is_same_quote_family = if quote == '\'' {
        is_power_shell_single_quote(character)
    } else {
        is_power_shell_double_quote(character)
    };
    is_line_start && character != quote && is_same_quote_family && chars.get(pos + 1) == Some(&'@')
}
