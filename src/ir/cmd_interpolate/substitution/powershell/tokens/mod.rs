//! Provide source-relative character lookaround for PowerShell recipe scanning.

use crate::shell_word::{is_power_shell_double_quote, is_power_shell_single_quote};

mod comments;
mod index_expressions;
mod member_expressions;
mod numeric_literals;
pub(super) use comments::comment_character_changes_scanner_state;

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
        // A line break ends the header; other PowerShell whitespace may precede it.
        while line_break.is_power_shell_whitespace() && !line_break.is_line_break() {
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

    /// Report whether a mid-token hash follows syntax that can begin a comment.
    pub(super) fn hash_follows_ambiguous_expression_boundary(
        self,
        has_variable_assignment_before_operator: bool,
    ) -> bool {
        if self.value() != '#' || self.is_token_boundary() {
            return false;
        }
        self.previous()
            .is_some_and(|previous| matches!(previous.value(), '=' | ']'))
            || self.hash_follows_unbraced_variable()
            || self.hash_follows_variable_member_access()
            || self.hash_follows_postfix_operator()
            || self.hash_follows_expression_operator(has_variable_assignment_before_operator)
            || numeric_literals::hash_follows_numeric_literal(self)
    }

    /// Report whether a hash follows an unescaped unbraced variable.
    fn hash_follows_unbraced_variable(self) -> bool {
        self.unescaped_variable_before().is_some() || self.hash_follows_automatic_variable()
    }

    /// Report whether a hash follows the complete `$$` automatic variable.
    fn hash_follows_automatic_variable(self) -> bool {
        let Some(variable_end) = self.previous() else {
            return false;
        };
        let Some(variable_start) = variable_end.previous() else {
            return false;
        };
        variable_end.value() == '$'
            && variable_start.value() == '$'
            && !variable_start.is_escaped_by_backtick()
    }

    /// Report whether a hash follows a property on a variable expression.
    fn hash_follows_variable_member_access(self) -> bool {
        let Some(mut member_character) = self.previous() else {
            return false;
        };
        if !member_character.is_variable_name_character() {
            return false;
        }
        while let Some(previous) = member_character.previous() {
            if !previous.is_variable_name_character() {
                break;
            }
            member_character = previous;
        }
        let Some(member_separator) = member_character.previous() else {
            return false;
        };
        if member_separator.value() != '.' {
            return false;
        }
        member_separator
            .previous()
            .is_some_and(Self::ends_with_unescaped_variable_expression)
    }

    /// Report whether this variable name immediately follows an unescaped `$`.
    fn is_unescaped_variable_name_start(self) -> bool {
        self.variable_name_start()
            .previous()
            .is_some_and(|dollar| dollar.value() == '$' && !dollar.is_escaped_by_backtick())
    }

    /// Mark postfix operators on simple, member, and indexed expressions.
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

    /// Catch redirection and assignment-expression operators before a hash.
    fn hash_follows_expression_operator(
        self,
        has_variable_assignment_before_operator: bool,
    ) -> bool {
        let Some(operator) = self.previous() else {
            return false;
        };
        match operator.value() {
            '>' => true,
            '+' | '*' | '/' | '%' | '-' => {
                has_variable_assignment_before_operator
                    || numeric_literals::operator_follows_numeric_literal(operator)
            }
            _ => false,
        }
    }

    /// Report whether the left-hand side before `=` is a PowerShell variable.
    pub(super) fn assignment_targets_variable(self) -> bool {
        let mut cursor = self.previous();
        while cursor.is_some_and(Self::is_power_shell_whitespace) {
            cursor = cursor.and_then(Self::previous);
        }
        let Some(mut name_character) = cursor else {
            return false;
        };
        if !name_character.is_variable_name_character() {
            return false;
        }
        while let Some(previous) = name_character.previous() {
            if !previous.is_variable_name_character() {
                break;
            }
            name_character = previous;
        }
        name_character
            .previous()
            .is_some_and(|dollar| dollar.value() == '$' && !dollar.is_escaped_by_backtick())
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

#[cfg(test)]
mod tests;
