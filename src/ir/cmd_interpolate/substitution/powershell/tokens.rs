//! Recognize PowerShell token boundaries relevant to safe recipe substitution.

use crate::shell_word::{is_power_shell_double_quote, is_power_shell_single_quote};

/// Report whether comment text can alter the partial lexical scanner.
pub(super) fn comment_character_changes_scanner_state(
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
        || starts_stop_parsing_token(chars, pos)
}

/// Return the quote character when a here-string opener starts at `pos`.
pub(super) fn here_string_opener(chars: &[char], pos: usize) -> Option<char> {
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

/// Report whether `ch` is a typographic PowerShell string delimiter.
pub(super) const fn is_typographic_quote(ch: char) -> bool {
    (is_power_shell_single_quote(ch) && ch != '\'')
        || (is_power_shell_double_quote(ch) && ch != '"')
}

/// Report whether an unquoted block-comment opener starts at `pos`.
pub(super) fn starts_block_comment(chars: &[char], pos: usize) -> bool {
    chars.get(pos) == Some(&'<') && chars.get(pos + 1) == Some(&'#')
}

/// Report whether a hash certainly starts a PowerShell comment.
pub(super) fn is_power_shell_comment_boundary(chars: &[char], pos: usize) -> bool {
    is_power_shell_token_boundary(chars, pos)
}

/// Report whether a here-string opener may follow a token or expression boundary.
pub(super) fn is_power_shell_here_string_boundary(chars: &[char], pos: usize) -> bool {
    is_power_shell_token_boundary(chars, pos)
        || pos
            .checked_sub(1)
            .and_then(|previous| chars.get(previous))
            .is_some_and(|ch| matches!(*ch, '=' | ']'))
}

/// Report whether `#` may begin a comment immediately after an unbraced variable.
pub(super) fn hash_follows_unbraced_variable(chars: &[char], pos: usize) -> bool {
    if chars.get(pos) != Some(&'#') || is_power_shell_comment_boundary(chars, pos) {
        return false;
    }
    unescaped_variable_before(chars, pos).is_some()
}

/// Report whether a mid-token hash may begin a comment in expression syntax.
pub(super) fn hash_follows_ambiguous_expression_boundary(chars: &[char], pos: usize) -> bool {
    if chars.get(pos) != Some(&'#') || is_power_shell_comment_boundary(chars, pos) {
        return false;
    }
    let follows_assignment_or_bracket = pos
        .checked_sub(1)
        .and_then(|previous| chars.get(previous))
        .is_some_and(|ch| matches!(*ch, '=' | ']'));
    follows_assignment_or_bracket
        || hash_follows_unbraced_variable(chars, pos)
        || hash_follows_postfix_operator(chars, pos)
}

/// Report whether `#` follows a postfix increment or decrement of a variable.
fn hash_follows_postfix_operator(chars: &[char], pos: usize) -> bool {
    let Some(operator_start) = pos.checked_sub(2) else {
        return false;
    };
    let Some(&operator) = chars.get(operator_start) else {
        return false;
    };
    matches!(operator, '+' | '-')
        && chars.get(operator_start + 1) == Some(&operator)
        && unescaped_variable_before(chars, operator_start).is_some()
}

/// Find an unescaped variable whose name ends immediately before `end`.
fn unescaped_variable_before(chars: &[char], end: usize) -> Option<usize> {
    let mut variable_start = end;
    while variable_start > 0
        && chars
            .get(variable_start - 1)
            .is_some_and(|ch| is_unbraced_variable_name_character(*ch))
    {
        variable_start -= 1;
    }
    if variable_start == end {
        return None;
    }
    let dollar_pos = variable_start.checked_sub(1)?;
    (chars.get(dollar_pos) == Some(&'$') && !is_escaped_by_backtick(chars, dollar_pos))
        .then_some(dollar_pos)
}

/// Recognize characters that can occur in an unbraced PowerShell variable name.
fn is_unbraced_variable_name_character(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | ':' | '?' | '^')
}

/// Report whether an odd run of backticks escapes the character at `pos`.
fn is_escaped_by_backtick(chars: &[char], pos: usize) -> bool {
    let mut cursor = pos;
    let mut is_escaped = false;
    while cursor > 0 && chars.get(cursor - 1) == Some(&'`') {
        cursor -= 1;
        is_escaped = !is_escaped;
    }
    is_escaped
}

/// Report whether PowerShell's exact `--%` token starts at `pos`.
pub(super) fn starts_stop_parsing_token(chars: &[char], pos: usize) -> bool {
    is_power_shell_token_boundary(chars, pos)
        && chars.get(pos) == Some(&'-')
        && chars.get(pos + 1) == Some(&'-')
        && chars.get(pos + 2) == Some(&'%')
        && chars
            .get(pos + 3)
            .is_none_or(|ch| is_power_shell_token_separator(*ch))
}

/// Report whether a source position begins a new PowerShell token.
pub(super) fn is_power_shell_token_boundary(chars: &[char], pos: usize) -> bool {
    pos == 0
        || chars
            .get(pos - 1)
            .is_some_and(|ch| is_power_shell_token_separator(*ch))
}

/// Match PowerShell whitespace without treating the unlisted U+0085 as one.
const fn is_power_shell_whitespace(ch: char) -> bool {
    ch.is_ascii_whitespace()
        || matches!(
            ch,
            '\u{000b}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
        )
}

/// Report whether a character separates PowerShell command tokens.
const fn is_power_shell_token_separator(ch: char) -> bool {
    is_power_shell_whitespace(ch)
        || matches!(
            ch,
            '\'' | '"' | '`' | ',' | ';' | '(' | ')' | '{' | '}' | '|' | '&'
        )
}

/// Report whether `pos` begins a Unix or Windows line break.
fn is_line_break_after(chars: &[char], pos: usize) -> bool {
    matches!(chars.get(pos), Some(&'\n'))
        || (chars.get(pos) == Some(&'\r') && chars.get(pos + 1) == Some(&'\n'))
}

#[cfg(test)]
mod tests {
    //! Verify which token boundaries allow a PowerShell comment to start.

    use super::hash_follows_ambiguous_expression_boundary;
    use super::is_power_shell_comment_boundary;
    use super::is_power_shell_here_string_boundary;
    use super::starts_stop_parsing_token;
    use rstest::rstest;

    #[rstest]
    #[case::after_space(' ', true)]
    #[case::after_nonbreaking_space('\u{00a0}', true)]
    #[case::after_ideographic_space('\u{3000}', true)]
    #[case::after_line_separator('\u{2028}', true)]
    #[case::after_next_line_control('\u{0085}', false)]
    #[case::after_assignment('=', false)]
    #[case::after_closing_bracket(']', false)]
    #[case::after_asterisk('*', false)]
    #[case::after_plus('+', false)]
    #[case::after_redirection('>', false)]
    fn classify_comment_boundaries(#[case] preceding: char, #[case] expected: bool) {
        let source = [preceding, '#'];

        assert_eq!(is_power_shell_comment_boundary(&source, 1), expected);
        assert!(is_power_shell_comment_boundary(&['#'], 0));
    }

    #[rstest]
    #[case::after_assignment('=', true)]
    #[case::after_closing_bracket(']', true)]
    #[case::after_space(' ', true)]
    #[case::after_asterisk('*', false)]
    fn classify_here_string_boundaries(#[case] preceding: char, #[case] expected: bool) {
        let source = [preceding, '@', '\''];

        assert_eq!(is_power_shell_here_string_boundary(&source, 1), expected);
    }

    #[rstest]
    #[case::after_postfix_increment("$i++#", true)]
    #[case::after_postfix_decrement("$i--#", true)]
    #[case::after_assignment("a=#", true)]
    #[case::after_bracket("a]#", true)]
    #[case::after_argument_asterisk("foo*#", false)]
    fn classify_ambiguous_expression_hashes(#[case] source: &str, #[case] expected: bool) {
        let characters = source.chars().collect::<Vec<_>>();
        let hash_position = characters.len() - 1;

        assert_eq!(
            hash_follows_ambiguous_expression_boundary(&characters, hash_position),
            expected
        );
    }

    #[test]
    fn recognize_stop_parsing_token_after_unicode_whitespace() {
        let characters = "\u{00a0}--%\u{3000}".chars().collect::<Vec<_>>();

        assert!(starts_stop_parsing_token(&characters, 1));
    }
}
