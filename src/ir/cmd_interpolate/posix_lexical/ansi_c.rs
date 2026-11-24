//! Recognise Bash ANSI-C string openings for both recipe traversals.
//!
//! Bash reads `$'…'` as an ANSI-C string, where a backslash escapes the next
//! character and only an unescaped apostrophe closes the string. Recipe markers
//! cannot be encoded safely in that region, so both traversals must identify it
//! before deciding whether a marker is eligible. The traversals own their quote
//! contexts; this module owns the lexical rules that decide when a dollar opens
//! the string, when a `$$` pair consumes the second dollar, and how a
//! backslash-newline continuation stays transparent between the two characters.

use super::PosixLexicalState;

/// Report whether the dollar at `pos` opens a Bash ANSI-C string.
///
/// The caller has already established that the dollar is unquoted and
/// unescaped. A continuation between the dollar and the quote is removed before
/// Bash tokenizes, so it is skipped here as well.
pub(super) fn opens_ansi_c_string(chars: &[char], pos: usize) -> bool {
    chars.get(pos) == Some(&'$') && next_significant_character(chars, pos + 1) == Some('\'')
}

/// Return the character Bash reads after removing line continuations.
fn next_significant_character(chars: &[char], mut pos: usize) -> Option<char> {
    loop {
        match chars.get(pos) {
            Some(&'\\') if chars.get(pos + 1) == Some(&'\n') => pos += 2,
            Some(&character) => return Some(character),
            None => return None,
        }
    }
}

/// One recipe character, paired with the source needed to look past it.
///
/// An ANSI-C opener spans two characters (`$` then `'`), possibly with a line
/// continuation between them, so the observer needs the whole sequence and the
/// current index rather than the character alone.
#[derive(Clone, Copy)]
pub(in crate::ir::cmd_interpolate) struct AnsiCCharacter<'a> {
    /// The full character sequence the traversal is scanning.
    chars: &'a [char],
    /// Index of the inspected character within `chars`.
    pos: usize,
    /// The character reaching quote tracking.
    ch: char,
}

impl<'a> AnsiCCharacter<'a> {
    /// Pair the character at `pos` with the sequence it belongs to.
    pub(in crate::ir::cmd_interpolate) const fn new(
        chars: &'a [char],
        pos: usize,
        ch: char,
    ) -> Self {
        Self { chars, pos, ch }
    }
}

impl PosixLexicalState {
    /// Advance ANSI-C opener state for one character reaching quote tracking.
    ///
    /// Returns `true` when the character is the apostrophe that opens an
    /// ANSI-C string. Only an unquoted dollar can start the sequence, and a
    /// second dollar in `$$` is consumed by the shell's PID expansion rather
    /// than starting one. A third dollar is free to do so again.
    pub(in crate::ir::cmd_interpolate) fn observe_ansi_c_character(
        &mut self,
        character: AnsiCCharacter<'_>,
        is_unquoted: bool,
    ) -> bool {
        let AnsiCCharacter { chars, pos, ch } = character;
        if !is_unquoted {
            self.reset_ansi_c_lookbehind();
            return false;
        }
        if ch == '\'' && self.pending_ansi_c_opener {
            self.reset_ansi_c_lookbehind();
            return true;
        }
        self.pending_ansi_c_opener = false;
        if ch == '$' {
            let is_second_of_pair = self.unpaired_dollar;
            self.unpaired_dollar = !is_second_of_pair;
            self.pending_ansi_c_opener = !is_second_of_pair && opens_ansi_c_string(chars, pos);
        } else {
            self.unpaired_dollar = false;
        }
        false
    }

    /// Advance ANSI-C state across one backslash-escaped character.
    ///
    /// A backslash-newline continuation is removed before Bash tokenizes, so it
    /// leaves a pending opener and an unpaired dollar intact. Every other
    /// escaped character is significant and ends both, because an escaped
    /// dollar cannot pair with the dollar after it.
    pub(in crate::ir::cmd_interpolate) const fn observe_ansi_c_escaped_character(
        &mut self,
        ch: char,
    ) {
        if ch != '\n' {
            self.reset_ansi_c_lookbehind();
        }
    }

    /// Forget any pending opener or `$$` half at a lexical boundary.
    ///
    /// Comments, heredoc bodies, and command-substitution regions all separate
    /// the characters either side of them, so a dollar before the boundary
    /// cannot pair with one after it.
    pub(in crate::ir::cmd_interpolate) const fn reset_ansi_c_lookbehind(&mut self) {
        self.pending_ansi_c_opener = false;
        self.unpaired_dollar = false;
    }
}
