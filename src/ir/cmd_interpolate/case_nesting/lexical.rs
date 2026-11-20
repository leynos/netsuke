//! Decide word boundaries and command positions for the `case` tracker.
//!
//! The tracker reads one character at a time, so every rule it applies is a
//! small lexical predicate: where a word ends, which operator starts a command,
//! and which reserved word leaves a command position open. None of them depends
//! on the tracker's own state, so they sit beside it rather than inside it.

use super::super::posix_lexical::SourceChar;

/// Report whether `ch` ends the unquoted word being accumulated.
///
/// A quote character ends the word as surely as a separator does: the quoted
/// text that follows is not word data, so a reserved word cannot continue into
/// it. Both categories are kept in one predicate because the caller treats them
/// identically.
pub(super) const fn ends_word(ch: char) -> bool {
    SourceChar::new(ch).is_word_terminator() || matches!(ch, '\'' | '"' | '`')
}

/// Report whether `ch` is an unquoted blank that leaves token position alone.
pub(super) const fn is_blank(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

/// Report whether a command may begin immediately after the operator `ch`.
///
/// A redirection operator introduces a filename rather than a command, so it is
/// deliberately absent: `esac` after `>` is a filename, not a keyword.
pub(super) const fn starts_command_after(ch: char) -> bool {
    matches!(ch, ';' | '&' | '|' | '\n' | '(' | ')')
}

/// Report whether a reserved word ends with the next word in command position.
///
/// These words open or continue a command list, so the following word is again
/// a command word. Any other word completes an argument list.
///
/// `{` belongs here because it opens a brace-group command list in POSIX and
/// Bash. A `case` written as the first command of such a group is a reserved
/// word exactly as it is at the start of the substitution, so overlooking `{`
/// would let the pattern terminator close the substitution early and lower a
/// marker in the arm for the enclosing quote context.
pub(super) fn starts_command_list(word: &str) -> bool {
    matches!(
        word,
        "if" | "then" | "else" | "elif" | "while" | "until" | "do" | "!" | "time" | "{"
    )
}

/// Report whether `word` may be an option of a `time` command.
///
/// Bash accepts `-p` and `--`; any other hyphen-led word is treated as an
/// option here too. Over-recognizing an option only keeps a command position
/// open longer, where a mistake causes a rejection rather than a mis-encoded
/// path.
pub(super) fn is_option_word(word: &str) -> bool {
    word.starts_with('-') && word != "-"
}
