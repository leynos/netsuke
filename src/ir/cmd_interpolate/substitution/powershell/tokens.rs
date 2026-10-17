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

/// Report whether a character separates PowerShell command tokens.
const fn is_power_shell_token_separator(ch: char) -> bool {
    ch.is_ascii_whitespace()
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
