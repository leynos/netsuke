//! Apply PowerShell quote and here-string rules during marker substitution.

use super::super::{IrGenError, QuoteContext, find_substitution, invalid_command_error};
use super::{MarkerProtection, SubstitutionTraversal};

/// Track the quote delimiter while a PowerShell here-string is active.
pub(super) struct PowerShellLexicalState {
    /// Retain the opener quote until its matching closing marker.
    here_string_quote: Option<char>,
    /// Fail closed after a typographic quote whose pairing is not modelled.
    has_typographic_quote: bool,
    /// Track comment text without treating its contents as shell syntax.
    in_unquoted_comment: bool,
    /// Fail closed after scanner-sensitive syntax inside an unparsed comment.
    has_ambiguous_comment_syntax: bool,
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
            has_typographic_quote: false,
            in_unquoted_comment: false,
            has_ambiguous_comment_syntax: false,
        }
    }

    /// Remember typographic quote characters so later markers fail closed.
    pub(super) const fn note_character(&mut self, ch: char) {
        self.has_typographic_quote |= is_typographic_quote(ch);
    }

    /// Report whether a typographic quote requires conservative marker rejection.
    pub(super) const fn has_typographic_quote(&self) -> bool {
        self.has_typographic_quote
    }

    /// Begin tracking a comment after an unquoted hash character.
    pub(super) const fn begin_unquoted_comment(&mut self) {
        self.in_unquoted_comment = true;
    }

    /// Fail closed after a block comment opener the partial scanner cannot parse.
    pub(super) const fn note_block_comment_opener(&mut self) {
        self.has_ambiguous_comment_syntax = true;
    }

    /// Report whether the current character belongs to an unquoted comment.
    pub(super) const fn is_in_unquoted_comment(&self) -> bool {
        self.in_unquoted_comment
    }

    /// Track syntax in a comment that could desynchronise the partial scanner.
    pub(super) const fn note_unquoted_comment_character(
        &mut self,
        changes_scanner_state: bool,
        is_line_break: bool,
    ) {
        self.has_ambiguous_comment_syntax |= changes_scanner_state;
        if is_line_break {
            self.in_unquoted_comment = false;
        }
    }

    /// Report whether comment syntax requires conservative marker rejection.
    pub(super) const fn has_ambiguous_comment_syntax(&self) -> bool {
        self.has_ambiguous_comment_syntax
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
        if self.power_shell_lexical.is_in_unquoted_comment() {
            let changes_scanner_state = comment_character_changes_scanner_state(
                self.chars,
                pos,
                ch,
                self.is_in_command_substitution(),
            );
            self.power_shell_lexical
                .note_unquoted_comment_character(changes_scanner_state, matches!(ch, '\r' | '\n'));
            return self.append_marker_for_context(
                pos,
                ch,
                Self::power_shell_marker_protection(self),
            );
        }
        if self.matches_unquoted_quote_context() && starts_block_comment(self.chars, pos) {
            self.power_shell_lexical.note_block_comment_opener();
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
        self.power_shell_lexical.note_character(ch);
        if ch == '#' && self.matches_unquoted_quote_context() {
            self.power_shell_lexical.begin_unquoted_comment();
        }
        if let Some(next) = self.append_power_shell_escaped_character(pos, ch)? {
            return Ok(next);
        }
        if self.append_power_shell_doubled_quote(pos, ch) {
            return Ok(pos + 2);
        }
        self.append_contextual_character(pos, ch, Self::power_shell_marker_protection)
    }

    /// Classify PowerShell quotes and command substitutions as protected regions.
    fn power_shell_marker_protection(&self) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            !matches!(self.active_quote_context(), QuoteContext::Unquoted)
                || self.power_shell_lexical.has_typographic_quote()
                || self.power_shell_lexical.has_ambiguous_comment_syntax()
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

    /// Preserve doubled quote characters inside a PowerShell string literal.
    fn append_power_shell_doubled_quote(&mut self, pos: usize, ch: char) -> bool {
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

/// Report whether one comment character could alter the partial scanner state.
fn comment_character_changes_scanner_state(
    chars: &[char],
    pos: usize,
    ch: char,
    is_in_command_substitution: bool,
) -> bool {
    matches!(ch, '\'' | '"' | '`')
        || is_typographic_quote(ch)
        || (ch == '$' && chars.get(pos + 1) == Some(&'('))
        || (is_in_command_substitution && matches!(ch, '(' | ')'))
        || here_string_opener(chars, pos).is_some()
}

/// Report whether an unquoted block-comment opener starts at `pos`.
fn starts_block_comment(chars: &[char], pos: usize) -> bool {
    chars.get(pos) == Some(&'<') && chars.get(pos + 1) == Some(&'#')
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

/// Report whether `ch` is a typographic PowerShell string delimiter.
const fn is_typographic_quote(ch: char) -> bool {
    matches!(
        ch,
        '\u{2018}'
            | '\u{2019}'
            | '\u{201a}'
            | '\u{201b}'
            | '\u{201c}'
            | '\u{201d}'
            | '\u{201e}'
            | '\u{201f}'
    )
}

/// Report whether `pos` begins a Unix or Windows line break.
fn is_line_break_after(chars: &[char], pos: usize) -> bool {
    matches!(chars.get(pos), Some(&'\n'))
        || (chars.get(pos) == Some(&'\r') && chars.get(pos + 1) == Some(&'\n'))
}
