//! Apply PowerShell quote and here-string rules during marker substitution.

use super::super::{IrGenError, QuoteContext, find_substitution, invalid_command_error};
use super::{MarkerProtection, SubstitutionTraversal};

/// Track the quote delimiter while a PowerShell here-string is active.
pub(super) struct PowerShellLexicalState {
    /// Retain the opener quote until its matching closing marker.
    here_string_quote: Option<char>,
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
    /// Initialise PowerShell here-string tracking for one traversal.
    pub(super) const fn new() -> Self {
        Self {
            here_string_quote: None,
        }
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
        if has_marker {
            return HereStringCharacter::Placeholder;
        }
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
    pub(super) fn append_power_shell_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<usize, IrGenError> {
        match self.power_shell_lexical.classify_here_string_character(
            self.chars,
            pos,
            find_substitution(self.chars, pos).is_some(),
        ) {
            HereStringCharacter::Outside => {}
            HereStringCharacter::Copied => {
                self.output.push(ch);
                return Ok(pos + 1);
            }
            HereStringCharacter::Closed(quote) => {
                self.output.push(quote);
                self.output.push('@');
                return Ok(pos + 2);
            }
            HereStringCharacter::Placeholder => {
                return Err(invalid_command_error(self.template.to_owned()));
            }
        }
        let is_unquoted_site = self.matches_unquoted_quote_context()
            && !self.in_backticks
            && !self.is_in_command_substitution();
        if let Some(quote) =
            self.power_shell_lexical
                .try_open_here_string(self.chars, pos, is_unquoted_site)
        {
            self.output.push('@');
            self.output.push(quote);
            return Ok(pos + 2);
        }
        if let Some(next) = self.append_power_shell_escaped_character(pos, ch)? {
            return Ok(next);
        }
        if self.append_power_shell_single_quote_escape(pos, ch) {
            return Ok(pos + 2);
        }
        self.append_contextual_character(pos, ch, Self::power_shell_marker_protection)
    }

    /// Classify PowerShell quotes and command substitutions as protected regions.
    fn power_shell_marker_protection(&self) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            !matches!(self.active_quote_context(), QuoteContext::Unquoted)
                || self.is_in_command_substitution(),
        )
    }

    /// Preserve a PowerShell backtick escape unless it would hide a recipe marker.
    fn append_power_shell_escaped_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
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

    /// Preserve a doubled PowerShell apostrophe inside its single-quoted literal.
    fn append_power_shell_single_quote_escape(&mut self, pos: usize, ch: char) -> bool {
        if !self.matches_single_quote_context() || !self.has_doubled_apostrophe(pos, ch) {
            return false;
        }
        self.output.push_str("''");
        true
    }

    /// Report whether `ch` begins a doubled PowerShell apostrophe escape.
    fn has_doubled_apostrophe(&self, pos: usize, ch: char) -> bool {
        ch == '\'' && self.chars.get(pos + 1) == Some(&'\'')
    }
}

/// Return the quote character when a here-string opener begins at `pos`.
fn here_string_opener(chars: &[char], pos: usize) -> Option<char> {
    if chars.get(pos) != Some(&'@') {
        return None;
    }
    let quote = *chars.get(pos + 1)?;
    if !matches!(quote, '\'' | '"') || !is_line_break_after(chars, pos + 2) {
        return None;
    }
    Some(quote)
}

/// Report whether a here-string closes at the start of the current line.
fn is_here_string_terminator(chars: &[char], pos: usize, quote: char) -> bool {
    let is_line_start = pos == 0 || chars.get(pos - 1) == Some(&'\n');
    is_line_start
        && chars.get(pos) == Some(&quote)
        && chars.get(pos + 1) == Some(&'@')
        && (pos + 2 == chars.len() || is_line_break_after(chars, pos + 2))
}

/// Report whether `pos` begins a Unix or Windows line break.
fn is_line_break_after(chars: &[char], pos: usize) -> bool {
    matches!(chars.get(pos), Some(&'\n'))
        || (chars.get(pos) == Some(&'\r') && chars.get(pos + 1) == Some(&'\n'))
}
