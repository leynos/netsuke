//! Provide source-relative character lookaround for PowerShell recipe scanning.

use crate::shell_word::{is_power_shell_double_quote, is_power_shell_single_quote};

/// Bind a character index to the source slice used for PowerShell lookaround.
///
/// Keeping the source and index together prevents token checks from receiving
/// mismatched buffers and offsets. Indices count Unicode scalar values because
/// the traversal already owns one `Vec<char>` for the template.
#[derive(Clone, Copy)]
pub(in crate::ir::cmd_interpolate::substitution) struct PowerShellSourceCharacter<'source> {
    /// Retain the character buffer used by this source position.
    characters: &'source [char],
    /// Identify the character within the buffer.
    position: usize,
    /// Cache the character validated at this source position.
    value: char,
}

/// Distinguish comments inside and outside PowerShell command substitutions.
#[derive(Clone, Copy)]
pub(super) enum PowerShellCommandSubstitutionContext {
    /// The comment is in ordinary script text.
    Outside,
    /// Parentheses in the comment may alter the enclosing `$()` scanner state.
    Inside,
}

impl<'source> PowerShellSourceCharacter<'source> {
    /// Create a source view when `position` identifies an existing character.
    pub(in crate::ir::cmd_interpolate::substitution) fn new(
        characters: &'source [char],
        position: usize,
    ) -> Option<Self> {
        let value = *characters.get(position)?;
        Some(Self {
            characters,
            position,
            value,
        })
    }

    /// Read the character at this valid source position.
    pub(super) const fn value(self) -> char {
        self.value
    }

    /// Report the character index represented by this view.
    pub(super) const fn position(self) -> usize {
        self.position
    }

    /// Move to the following source character when it exists.
    pub(super) fn next(self) -> Option<Self> {
        Self::new(self.characters, self.position.checked_add(1)?)
    }

    /// Move to the preceding source character when it exists.
    pub(super) fn previous(self) -> Option<Self> {
        Self::new(self.characters, self.position.checked_sub(1)?)
    }

    /// Report whether the current character is a CR or LF line break.
    pub(super) const fn is_line_break(self) -> bool {
        matches!(self.value(), '\r' | '\n')
    }

    /// Report whether the current character is a typographic string delimiter.
    pub(super) const fn is_typographic_quote(self) -> bool {
        let character = self.value();
        (is_power_shell_single_quote(character) && character != '\'')
            || (is_power_shell_double_quote(character) && character != '"')
    }

    /// Report whether this position begins a PowerShell here-string header.
    pub(super) fn here_string_opener(self) -> Option<char> {
        if self.value() != '@' {
            return None;
        }
        let quote_position = self.next()?;
        let quote = quote_position.value();
        if !matches!(quote, '\'' | '"') {
            return None;
        }
        let mut line_break = quote_position.next()?;
        // PowerShell permits horizontal whitespace before the header newline.
        while matches!(line_break.value(), ' ' | '\t') {
            line_break = line_break.next()?;
        }
        line_break.is_line_break_sequence().then_some(quote)
    }

    /// Report whether a CR or LF sequence starts at this position.
    fn is_line_break_sequence(self) -> bool {
        self.value() == '\n'
            || (self.value() == '\r' && self.next().is_some_and(|next| next.value() == '\n'))
    }

    /// Report whether an unquoted block-comment opener starts here.
    pub(super) fn starts_block_comment(self) -> bool {
        self.value() == '<' && self.next().is_some_and(|next| next.value() == '#')
    }

    /// Report whether this source character begins a PowerShell token.
    pub(super) fn is_token_boundary(self) -> bool {
        self.previous().is_none_or(Self::is_token_separator)
    }

    /// Report whether this position can begin a here-string expression.
    pub(super) fn is_here_string_boundary(self) -> bool {
        self.is_token_boundary()
            || self
                .previous()
                .is_some_and(|previous| matches!(previous.value(), '=' | ']'))
    }

    /// Report whether a hash may start a comment after an expression boundary.
    pub(super) fn hash_follows_ambiguous_expression_boundary(self) -> bool {
        if self.value() != '#' || self.is_token_boundary() {
            return false;
        }
        self.previous()
            .is_some_and(|previous| matches!(previous.value(), '=' | ']'))
            || self.hash_follows_unbraced_variable()
            || self.hash_follows_postfix_operator()
    }

    /// Report whether a hash follows an unescaped unbraced variable.
    fn hash_follows_unbraced_variable(self) -> bool {
        self.value() == '#'
            && !self.is_token_boundary()
            && self.unescaped_variable_before().is_some()
    }

    /// Mark every hash after `++` or `--` as ambiguous.
    ///
    /// PowerShell postfix operands include indexed and member expressions, so
    /// recognizing only bare variables would leave comment quotes untracked.
    fn hash_follows_postfix_operator(self) -> bool {
        let Some(second_operator_character) = self.previous() else {
            return false;
        };
        let Some(first_operator_character) = second_operator_character.previous() else {
            return false;
        };
        let operator = first_operator_character.value();
        matches!(operator, '+' | '-') && second_operator_character.value() == operator
    }

    /// Find the unescaped variable whose name ends immediately before here.
    fn unescaped_variable_before(self) -> Option<Self> {
        let mut variable_character = self.previous()?;
        if !variable_character.is_variable_name_character() {
            return None;
        }
        while let Some(previous) = variable_character.previous() {
            if !previous.is_variable_name_character() {
                break;
            }
            variable_character = previous;
        }
        let dollar = variable_character.previous()?;
        (dollar.value() == '$' && !dollar.is_escaped_by_backtick()).then_some(dollar)
    }

    /// Report whether this character can occur in an unbraced variable name.
    fn is_variable_name_character(self) -> bool {
        let character = self.value();
        character.is_alphanumeric() || matches!(character, '_' | ':' | '?' | '^')
    }

    /// Report whether an odd run of preceding backticks escapes this position.
    fn is_escaped_by_backtick(self) -> bool {
        let mut cursor = self;
        let mut is_escaped = false;
        while let Some(previous) = cursor.previous() {
            if previous.value() != '`' {
                break;
            }
            is_escaped = !is_escaped;
            cursor = previous;
        }
        is_escaped
    }

    /// Report whether PowerShell's exact standalone `--%` token starts here.
    pub(super) fn starts_stop_parsing_token(self) -> bool {
        if self.value() != '-' || !self.is_token_boundary() {
            return false;
        }
        let Some(second_hyphen) = self.next() else {
            return false;
        };
        let Some(percent) = second_hyphen.next() else {
            return false;
        };
        second_hyphen.value() == '-'
            && percent.value() == '%'
            && percent.next().is_none_or(Self::is_token_separator)
    }

    /// Report whether a source character separates PowerShell command tokens.
    const fn is_token_separator(self) -> bool {
        let character = self.value();
        self.is_power_shell_whitespace()
            || matches!(
                character,
                '\'' | '"' | '`' | ',' | ';' | '(' | ')' | '{' | '}' | '|' | '&'
            )
    }

    /// Match PowerShell whitespace without treating the unlisted U+0085 as one.
    const fn is_power_shell_whitespace(self) -> bool {
        let character = self.value();
        character.is_ascii_whitespace()
            || matches!(
                character,
                '\u{000b}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'
                    ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
            )
    }
}

/// Report whether comment text can alter the partial lexical scanner.
pub(super) fn comment_character_changes_scanner_state(
    character: PowerShellSourceCharacter<'_>,
    context: PowerShellCommandSubstitutionContext,
) -> bool {
    matches!(character.value(), '\'' | '"' | '`')
        || character.is_typographic_quote()
        || (character.value() == '$'
            && character.next().is_some_and(|next| next.value() == '('))
        || (matches!(context, PowerShellCommandSubstitutionContext::Inside)
            && matches!(character.value(), '(' | ')'))
        // Deliberately over-reject here-string-looking text inside comments.
        || character.here_string_opener().is_some()
        || character.starts_stop_parsing_token()
}

#[cfg(test)]
mod tests {
    //! Verify PowerShell token recognition uses source-relative character positions.

    use super::PowerShellSourceCharacter;
    use rstest::rstest;

    struct TokenScenario {
        source: &'static str,
        position: usize,
        expected: bool,
    }

    #[rstest]
    #[case::after_space(TokenScenario { source: " #", position: 1, expected: true })]
    #[case::after_nonbreaking_space(TokenScenario { source: "\u{00a0}#", position: 1, expected: true })]
    #[case::after_ideographic_space(TokenScenario { source: "\u{3000}#", position: 1, expected: true })]
    #[case::after_line_separator(TokenScenario { source: "\u{2028}#", position: 1, expected: true })]
    #[case::after_next_line_control(TokenScenario { source: "\u{0085}#", position: 1, expected: false })]
    #[case::after_assignment(TokenScenario { source: "=#", position: 1, expected: false })]
    #[case::after_closing_bracket(TokenScenario { source: "]#", position: 1, expected: false })]
    #[case::after_asterisk(TokenScenario { source: "*#", position: 1, expected: false })]
    #[case::after_plus(TokenScenario { source: "+#", position: 1, expected: false })]
    #[case::after_redirection(TokenScenario { source: ">#", position: 1, expected: false })]
    fn classify_comment_boundaries(#[case] scenario: TokenScenario) {
        let characters = scenario.source.chars().collect::<Vec<_>>();
        let position = PowerShellSourceCharacter::new(&characters, scenario.position)
            .expect("the scenario position identifies a source character");

        assert_eq!(position.is_token_boundary(), scenario.expected);
        let first =
            PowerShellSourceCharacter::new(&['#'], 0).expect("the first source character exists");
        assert!(first.is_token_boundary());
    }

    #[rstest]
    #[case::after_assignment(TokenScenario { source: "=@'", position: 1, expected: true })]
    #[case::after_closing_bracket(TokenScenario { source: "]@'", position: 1, expected: true })]
    #[case::after_space(TokenScenario { source: " @'", position: 1, expected: true })]
    #[case::after_asterisk(TokenScenario { source: "*@'", position: 1, expected: false })]
    fn classify_here_string_boundaries(#[case] scenario: TokenScenario) {
        let characters = scenario.source.chars().collect::<Vec<_>>();
        let position = PowerShellSourceCharacter::new(&characters, scenario.position)
            .expect("the scenario position identifies a source character");

        assert_eq!(position.is_here_string_boundary(), scenario.expected);
    }

    #[rstest]
    #[case::after_postfix_increment(TokenScenario { source: "$i++#", position: 4, expected: true })]
    #[case::after_postfix_decrement(TokenScenario { source: "$i--#", position: 4, expected: true })]
    #[case::after_member_postfix_increment(TokenScenario { source: "$obj.Value++#", position: 12, expected: true })]
    #[case::after_member_postfix_decrement(TokenScenario { source: "$obj.Value--#", position: 12, expected: true })]
    #[case::after_indexed_postfix_increment(TokenScenario { source: "$arr[0]++#", position: 9, expected: true })]
    #[case::after_indexed_postfix_decrement(TokenScenario { source: "$obj.Items[0]--#", position: 15, expected: true })]
    #[case::after_assignment(TokenScenario { source: "a=#", position: 2, expected: true })]
    #[case::after_bracket(TokenScenario { source: "a]#", position: 2, expected: true })]
    #[case::after_argument_asterisk(TokenScenario { source: "foo*#", position: 4, expected: false })]
    fn classify_ambiguous_expression_hashes(#[case] scenario: TokenScenario) {
        let characters = scenario.source.chars().collect::<Vec<_>>();
        let position = PowerShellSourceCharacter::new(&characters, scenario.position)
            .expect("the scenario position identifies a source character");

        assert_eq!(
            position.hash_follows_ambiguous_expression_boundary(),
            scenario.expected
        );
    }

    #[test]
    fn recognize_stop_parsing_token_after_unicode_whitespace() {
        let characters = "\u{00a0}--%\u{3000}".chars().collect::<Vec<_>>();
        let position = PowerShellSourceCharacter::new(&characters, 1)
            .expect("the token begins at the expected character index");

        assert!(position.starts_stop_parsing_token());
    }

    #[test]
    fn source_positions_keep_unicode_character_indices_and_source_edges() {
        let characters = "é --%".chars().collect::<Vec<_>>();
        let delimiter = PowerShellSourceCharacter::new(&characters, 1)
            .expect("space follows the non-ASCII character");
        let token = delimiter.next().expect("the token follows the space");
        let final_character = PowerShellSourceCharacter::new(&characters, 4)
            .expect("the source has a final character");

        assert_eq!(token.value(), '-');
        assert_eq!(token.position(), 2);
        assert!(token.starts_stop_parsing_token());
        assert!(PowerShellSourceCharacter::new(&characters, 5).is_none());
        assert!(
            PowerShellSourceCharacter::new(&characters, 0)
                .expect("the source starts with a character")
                .previous()
                .is_none()
        );
        assert!(final_character.next().is_none());
    }
}
