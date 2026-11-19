//! Decide where POSIX permits a comment to begin.
//!
//! POSIX starts a comment only where a token may start, so the tracker must
//! recognise unescaped separators while ignoring line continuations. Keeping
//! these predicates together leaves the parent module to own the traversal
//! state that consults them.

use super::SourceChar;

/// Report whether `pos` follows a boundary where POSIX permits a comment.
///
/// POSIX starts a comment only where a token may start: at the beginning of
/// input or after an unescaped separator. Every other character is word data,
/// so Unicode spaces, carriage return, vertical tab, and form feed do not
/// begin a comment and do not stop quote tracking. A `)` that closes a command
/// substitution also remains in the surrounding word, so the caller passes its
/// position as `closed_substitution` to suppress that false boundary.
pub(super) fn is_shell_word_boundary(
    chars: &[char],
    pos: usize,
    closed_substitution: Option<usize>,
) -> bool {
    let Some(previous_pos) = preceding_shell_character(chars, pos) else {
        return true;
    };
    if closed_substitution == Some(previous_pos) {
        return false;
    }
    is_unescaped_character(chars, pos)
        && is_unescaped_character(chars, previous_pos)
        && chars
            .get(previous_pos)
            .is_some_and(|previous| SourceChar::new(*previous).is_word_terminator())
}

/// Find the preceding shell character after removing unquoted line continuations.
fn preceding_shell_character(chars: &[char], pos: usize) -> Option<usize> {
    let mut end = pos;
    loop {
        let previous = end.checked_sub(1)?;
        if chars.get(previous) != Some(&'\n') || is_unescaped_character(chars, previous) {
            return Some(previous);
        }
        end = previous.checked_sub(1)?;
    }
}

/// Report whether the character at `pos` is not escaped by a backslash run.
pub(super) fn is_unescaped_character(chars: &[char], pos: usize) -> bool {
    chars
        .get(..pos)
        .into_iter()
        .flatten()
        .rev()
        .take_while(|character| **character == '\\')
        .count()
        .rem_euclid(2)
        == 0
}
