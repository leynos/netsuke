//! Track PowerShell lexical state that can make marker substitution unsafe.

mod comments;

use super::super::super::{
    INS_TOKEN, IrGenError, OUTS_TOKEN, QuoteContext, find_substitution, invalid_command_error,
};
use super::super::{MarkerProtection, SubstitutionTraversal};
use super::tokens::PowerShellSourceCharacter;
use crate::shell_word::{is_power_shell_double_quote, is_power_shell_single_quote};

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
    pub(in crate::ir::cmd_interpolate::substitution) const fn note_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) {
        self.has_typographic_quote |= character.is_typographic_quote();
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
        character: PowerShellSourceCharacter<'_>,
        has_marker: bool,
    ) -> HereStringCharacter {
        let Some(quote) = self.here_string_quote else {
            return HereStringCharacter::Outside;
        };
        if is_here_string_terminator(character, quote) {
            self.here_string_quote = None;
            return HereStringCharacter::Closed(quote);
        }
        if is_typographic_here_string_terminator(character, quote) {
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
        character: PowerShellSourceCharacter<'_>,
        is_unquoted_site: bool,
    ) -> Option<char> {
        if !is_unquoted_site {
            return None;
        }
        let quote = character.here_string_opener()?;
        self.here_string_quote = Some(quote);
        Some(quote)
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Preserve PowerShell text while lowering only manifest-owned markers.
    pub(in crate::ir::cmd_interpolate::substitution) fn append_power_shell_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Result<usize, IrGenError> {
        let pos = character.position();
        let ch = character.value();
        if self.power_shell_stop_parsing.is_active() {
            return self.append_stop_parsing_character(character);
        }
        if let Some(next) = self.append_inert_power_shell_character(character)? {
            return Ok(next);
        }
        self.power_shell_lexical.note_character(character);
        // Track ambiguous expression hashes because their suffix may be a comment.
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

    /// Preserve characters whose lexical state bypasses marker scanning.
    fn append_inert_power_shell_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Result<Option<usize>, IrGenError> {
        if let Some(next) = self.append_here_string_character(character)? {
            return Ok(Some(next));
        }
        if self.power_shell_lexical.is_in_unquoted_comment() {
            return self.append_unquoted_comment_character(character).map(Some);
        }
        if self.power_shell_lexical.is_in_ambiguous_comment() {
            self.note_ambiguous_comment_character(character);
        }
        self.note_unparsed_block_comment_opener(character);
        self.note_unmodelled_power_shell_syntax(character);
        Ok(self.open_here_string(character))
    }

    /// Preserve or reject a character in an active PowerShell here-string.
    fn append_here_string_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Result<Option<usize>, IrGenError> {
        let pos = character.position();
        match self
            .power_shell_lexical
            .classify_here_string_character(character, find_substitution(self.chars, pos).is_some())
        {
            HereStringCharacter::Outside => Ok(None),
            HereStringCharacter::Copied => {
                self.output.push(character.value());
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
    fn open_here_string(&mut self, character: PowerShellSourceCharacter<'_>) -> Option<usize> {
        let is_unquoted_site = self.matches_unquoted_quote_context()
            && !self.in_backticks
            && !self.is_in_command_substitution();
        if Self::is_mid_token_here_string_opener(is_unquoted_site, character) {
            // A mid-token `@'` is a quoted word part, not a here-string.
            self.power_shell_lexical.note_unmodelled_syntax();
            return None;
        }
        let quote = self
            .power_shell_lexical
            .try_open_here_string(character, is_unquoted_site)?;
        self.output.push('@');
        self.output.push(quote);
        Some(character.position() + 2)
    }

    /// Identify a here-string opener embedded in a PowerShell token.
    fn is_mid_token_here_string_opener(
        is_unquoted_site: bool,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        is_unquoted_site
            && character.here_string_opener().is_some()
            && !character.is_here_string_boundary()
    }

    /// Fail closed when syntax can desynchronize the partial PowerShell scanner.
    fn note_unmodelled_power_shell_syntax(&mut self, character: PowerShellSourceCharacter<'_>) {
        if self.has_unmodelled_power_shell_syntax(character) {
            self.power_shell_lexical.note_unmodelled_syntax();
        }
        if self.starts_power_shell_stop_parsing_token(character) {
            self.power_shell_stop_parsing.begin();
        }
    }

    /// Classify syntax that can invalidate the scanner's quote and token model.
    fn has_unmodelled_power_shell_syntax(&self, character: PowerShellSourceCharacter<'_>) -> bool {
        self.has_here_string_in_command_substitution(character)
            || self.has_braced_variable_name(character)
    }

    /// Detect a here-string opener inside a nested command substitution.
    fn has_here_string_in_command_substitution(
        &self,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        self.is_in_command_substitution()
            && self.matches_unquoted_quote_context()
            && character.here_string_opener().is_some()
    }

    /// Detect a braced variable whose quote rules are not modelled here.
    fn has_braced_variable_name(&self, character: PowerShellSourceCharacter<'_>) -> bool {
        character.value() == '$'
            && character.next().is_some_and(|next| next.value() == '{')
            && !self.matches_single_quote_context()
    }

    /// Detect an unquoted, unescaped PowerShell stop-parsing token.
    fn starts_power_shell_stop_parsing_token(
        &self,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        !self.in_backticks
            && self.matches_unquoted_quote_context()
            && character.starts_stop_parsing_token()
    }

    /// Classify PowerShell quotes and command substitutions as protected regions.
    fn power_shell_marker_protection(&self, pos: usize) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            !matches!(self.active_quote_context(), QuoteContext::Unquoted)
                || self.power_shell_lexical.has_typographic_quote()
                || self.power_shell_lexical.has_unmodelled_syntax()
                || self.is_in_command_substitution()
                || self.marker_has_ambiguous_adjacency(pos),
        )
    }

    /// Reject a path literal that would merge with a quoted part or another marker.
    fn marker_has_ambiguous_adjacency(&self, pos: usize) -> bool {
        let Some((_, marker_length)) = find_substitution(self.chars, pos) else {
            return false;
        };
        let marker_end = pos + marker_length;
        let touches_single_quote = pos
            .checked_sub(1)
            .and_then(|previous| self.chars.get(previous))
            .is_some_and(|&character| is_power_shell_single_quote(character))
            || self
                .chars
                .get(marker_end)
                .is_some_and(|&character| is_power_shell_single_quote(character));
        touches_single_quote || self.marker_follows_another_marker(pos)
    }

    /// Detect a marker that starts immediately after another PowerShell marker.
    fn marker_follows_another_marker(&self, pos: usize) -> bool {
        [INS_TOKEN.len(), OUTS_TOKEN.len()]
            .into_iter()
            .any(|length| {
                pos.checked_sub(length).is_some_and(|previous_start| {
                    find_substitution(self.chars, previous_start)
                        .is_some_and(|(_, previous_length)| previous_start + previous_length == pos)
                })
            })
    }

    /// Preserve a PowerShell backtick escape unless it would hide a recipe marker.
    fn append_power_shell_escaped_character(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> Result<Option<usize>, IrGenError> {
        let pos = character.position();
        let ch = character.value();
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
    fn append_power_shell_doubled_quote(
        &mut self,
        character: PowerShellSourceCharacter<'_>,
    ) -> bool {
        let pos = character.position();
        let ch = character.value();
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
fn is_here_string_terminator(character: PowerShellSourceCharacter<'_>, quote: char) -> bool {
    let is_line_start = character
        .previous()
        .is_none_or(|previous| previous.value() == '\n');
    is_line_start
        && character.value() == quote
        && character.next().is_some_and(|next| next.value() == '@')
}

/// Report whether a typographic quote from the opener's family may close the body.
fn is_typographic_here_string_terminator(
    position: PowerShellSourceCharacter<'_>,
    quote: char,
) -> bool {
    let is_line_start = position
        .previous()
        .is_none_or(|previous| previous.value() == '\n');
    let is_same_quote_family = if quote == '\'' {
        is_power_shell_single_quote(position.value())
    } else {
        is_power_shell_double_quote(position.value())
    };
    is_line_start
        && position.value() != quote
        && is_same_quote_family
        && position.next().is_some_and(|next| next.value() == '@')
}
