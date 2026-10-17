//! Apply PowerShell quote and here-string rules during marker substitution.

use super::super::{IrGenError, QuoteContext, find_substitution, invalid_command_error};
use super::{MarkerProtection, SubstitutionTraversal};

/// Group a PowerShell source offset with the character being traversed.
///
/// Keep this carrier local to the PowerShell traversal; it does not model a
/// general shell character or replace the POSIX scanner's character type.
#[derive(Clone, Copy)]
pub(super) struct PowerShellSourceCharacter {
    /// Identify the source position used for lexical lookahead.
    pub(super) position: usize,
    /// Preserve the character at that position.
    pub(super) value: char,
}

/// Track the quote delimiter while a PowerShell here-string is active.
pub(super) struct PowerShellLexicalState {
    /// Retain the opener quote until its matching closing marker.
    here_string_quote: Option<char>,
    /// Fail closed after a typographic quote whose pairing is not modelled.
    has_typographic_quote: bool,
    /// Track comment text without treating its contents as shell syntax.
    in_unquoted_comment: bool,
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
    pub(super) const fn new() -> Self {
        Self {
            here_string_quote: None,
            has_typographic_quote: false,
            in_unquoted_comment: false,
            has_unmodelled_syntax: false,
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

    /// Fail closed after syntax the partial scanner cannot safely parse.
    pub(super) const fn note_unmodelled_syntax(&mut self) {
        self.has_unmodelled_syntax = true;
    }

    /// Report whether the current character belongs to an unquoted comment.
    pub(super) const fn is_in_unquoted_comment(&self) -> bool {
        self.in_unquoted_comment
    }

    /// Track syntax in a comment that could desynchronize the partial scanner.
    pub(super) const fn note_unquoted_comment_character(
        &mut self,
        changes_scanner_state: bool,
        is_line_break: bool,
    ) {
        self.has_unmodelled_syntax |= changes_scanner_state;
        if is_line_break {
            self.in_unquoted_comment = false;
        }
    }

    /// Report whether unmodelled syntax requires conservative marker rejection.
    pub(super) const fn has_unmodelled_syntax(&self) -> bool {
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
    pub(super) fn append_power_shell_character(
        &mut self,
        character: PowerShellSourceCharacter,
    ) -> Result<usize, IrGenError> {
        let pos = character.position;
        let ch = character.value;
        if let Some(next) = self.append_here_string_character(character)? {
            return Ok(next);
        }
        if self.power_shell_lexical.is_in_unquoted_comment() {
            return self.append_unquoted_comment_character(character);
        }
        self.note_unparsed_block_comment_opener(character);
        if let Some(next) = self.open_here_string(character) {
            return Ok(next);
        }
        self.power_shell_lexical.note_character(ch);
        if ch == '#' && self.matches_unquoted_quote_context() {
            self.power_shell_lexical.begin_unquoted_comment();
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

    /// Track ambiguous syntax and preserve a PowerShell comment character.
    fn append_unquoted_comment_character(
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
    fn note_unparsed_block_comment_opener(&mut self, character: PowerShellSourceCharacter) {
        if self.matches_unquoted_quote_context()
            && starts_block_comment(self.chars, character.position)
        {
            self.power_shell_lexical.note_unmodelled_syntax();
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

    /// Fail closed when syntax can desynchronize the partial PowerShell scanner.
    fn note_unmodelled_power_shell_syntax(&mut self, character: PowerShellSourceCharacter) {
        let position = character.position;
        let here_string_in_command_substitution = self.is_in_command_substitution()
            && self.matches_unquoted_quote_context()
            && here_string_opener(self.chars, position).is_some();
        let braced_variable_name = character.value == '$'
            && self.chars.get(position + 1) == Some(&'{')
            && !self.matches_single_quote_context();
        if here_string_in_command_substitution || braced_variable_name {
            self.power_shell_lexical.note_unmodelled_syntax();
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
        // Deliberately over-reject here-string-looking text inside comments.
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
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let mut line_break_pos = pos + 2;
    // PowerShell's tokenizer allows horizontal whitespace before the header newline.
    while matches!(chars.get(line_break_pos), Some(' ' | '\t')) {
        line_break_pos += 1;
    }
    if !is_line_break_after(chars, line_break_pos) {
        return None;
    }
    Some(quote)
}

/// Report whether a here-string closes at the start of the current line.
fn is_here_string_terminator(chars: &[char], pos: usize, quote: char) -> bool {
    let is_line_start = pos == 0 || chars.get(pos - 1) == Some(&'\n');
    is_line_start && chars.get(pos) == Some(&quote) && chars.get(pos + 1) == Some(&'@')
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
