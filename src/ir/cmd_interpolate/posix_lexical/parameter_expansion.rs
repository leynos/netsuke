//! Track `${ }` parameter expansions while copying plain POSIX shell text.
//!
//! A `${ }` expansion can hold a `#` that follows a blank, as the pattern
//! removal in `${name# #}` does, so comment recognition is suspended while one
//! is open. The count is scoped to the region that opened the expansion,
//! because a brace inside a command substitution — `$( )` or backticks —
//! belongs to that substitution. The GNU Bash manual records that a
//! substitution's braces do not match an enclosing `${`, and bash parses a
//! backtick body as its own command region just as it does a `$( )` body. One
//! shared counter would let such a brace close an expansion the shell still
//! holds open, and a number sign after it would then copy the rest of the line,
//! including a heredoc declared on it, so marker lowering would resume in body
//! lines the shell still expands.

use super::{PosixLexicalState, SourceChar, is_shell_word_boundary, is_unescaped_character};

impl PosixLexicalState {
    /// Record the `${ }` nesting one character of plain text opens or closes.
    ///
    /// A caller passes only characters from plain executable text, which is the
    /// same region where comment recognition runs. Quoted and inert regions
    /// need no tracking of their own: a brace there is not expansion syntax and
    /// no comment can start, so brace balance is decided entirely in one region.
    ///
    /// The count is what stops `${name# #}` from looking like a comment.
    /// Reading that number sign as a comment would copy the rest of the line
    /// verbatim, including a heredoc declared on it, and marker lowering would
    /// then resume in body lines the shell still expands.
    pub(in crate::ir::cmd_interpolate) fn note_parameter_expansion(
        &mut self,
        chars: &[char],
        pos: usize,
        ch: SourceChar,
    ) {
        match ch.as_char() {
            '}' => self.record_closing_brace(chars, pos),
            '{' if opens_parameter_expansion(chars, pos) => {
                self.parameter_expansion_depth += 1;
            }
            _ => {}
        }
    }

    /// Report whether an unquoted number sign begins a POSIX comment.
    ///
    /// Two rules decide it. A `#` inside an open `${ }` parameter expansion is
    /// expansion syntax, so only plain text outside every expansion can begin a
    /// comment. `closed_substitution` then names the `)` that closed a command
    /// substitution when the caller tracks one: that `)` remains part of the
    /// surrounding word, so a following `#` is word data rather than a comment.
    pub(in crate::ir::cmd_interpolate) fn starts_comment(
        &self,
        chars: &[char],
        pos: usize,
        ch: SourceChar,
    ) -> bool {
        ch.as_char() == '#'
            && self.parameter_expansion_depth == 0
            && is_shell_word_boundary(chars, pos, self.closed_command_substitution)
    }

    /// End a POSIX comment at a delimiter rather than at a newline.
    pub(in crate::ir::cmd_interpolate) const fn end_comment(&mut self) {
        self.in_comment = false;
    }

    /// Report whether the backtick at `pos` closes a comment opened inside it.
    ///
    /// Bash locates a backtick substitution's end before it parses the body, so
    /// a `#` comment inside the body ends at the closing backtick instead of
    /// running to the end of the enclosing line. Continuing the comment past
    /// that backtick copies the outer declaration as comment text, so the
    /// heredoc is never queued and marker lowering resumes in a body the shell
    /// still expands.
    pub(in crate::ir::cmd_interpolate) fn backtick_closes_comment(
        &self,
        chars: &[char],
        pos: usize,
        ch: SourceChar,
    ) -> bool {
        self.in_comment && ch.as_char() == '`' && is_unescaped_character(chars, pos)
    }

    /// Start counting `${ }` nesting for one new command-substitution region.
    ///
    /// Both spellings, `$( )` and backticks, delimit a region bash parses as
    /// its own command list. The enclosing count is saved rather than
    /// discarded: the region's own braces are balanced within it, so the outer
    /// count must be exactly what it was once the region closes.
    pub(in crate::ir::cmd_interpolate) fn enter_command_substitution_region(&mut self) {
        self.enclosing_parameter_expansion_depths
            .push(self.parameter_expansion_depth);
        self.parameter_expansion_depth = 0;
    }

    /// Resume the enclosing region's `${ }` count as one region closes.
    ///
    /// A `${` left open inside the region is dropped with it. That state only
    /// arises from text the shell itself rejects, and keeping it would suppress
    /// comment recognition in the enclosing region.
    pub(in crate::ir::cmd_interpolate) fn leave_command_substitution_region(&mut self) {
        if let Some(enclosing) = self.enclosing_parameter_expansion_depths.pop() {
            self.parameter_expansion_depth = enclosing;
        }
    }

    /// Close one open `${ }` expansion when an unescaped brace ends it.
    fn record_closing_brace(&mut self, chars: &[char], pos: usize) {
        if self.parameter_expansion_depth > 0 && is_unescaped_character(chars, pos) {
            self.parameter_expansion_depth -= 1;
        }
    }
}

/// Report whether the brace at `pos` opens a `${ }` parameter expansion.
fn opens_parameter_expansion(chars: &[char], pos: usize) -> bool {
    let Some(dollar) = preceding_logical_character(chars, pos) else {
        return false;
    };
    chars.get(dollar) == Some(&'$')
        && is_unescaped_character(chars, dollar)
        && is_unescaped_character(chars, pos)
}

/// Return the character the shell reads immediately before `pos`.
///
/// The shell removes a backslash-newline pair before it lexes an expansion, so
/// `$\` followed by a newline and `{` opens `${ }`. Unlike
/// `preceding_shell_character`, which reports the last character a word keeps,
/// this steps over the removed pair itself: the caller needs the character that
/// ends up adjacent to the brace.
fn preceding_logical_character(chars: &[char], pos: usize) -> Option<usize> {
    let mut previous = pos.checked_sub(1)?;
    while chars.get(previous) == Some(&'\n') && !is_unescaped_character(chars, previous) {
        previous = previous.checked_sub(2)?;
    }
    Some(previous)
}
