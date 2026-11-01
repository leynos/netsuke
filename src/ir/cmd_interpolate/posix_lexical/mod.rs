//! Preserve POSIX comments and heredoc data during recipe interpolation.
//!
//! The interpolation traversal must not treat quote-like characters inside
//! inert shell regions as syntax. This module recognises just enough POSIX
//! lexical structure to retain those regions byte-for-byte while the caller
//! lowers markers in executable shell text.

use std::collections::VecDeque;

use super::{CommandBindings, find_substitution};

mod delimiter;

use delimiter::{parse_delimiter, render_delimiter};

#[cfg(test)]
mod tests;

/// Track POSIX regions whose contents are not executable shell syntax.
pub(super) struct PosixLexicalState {
    /// Record whether text through the next newline is a shell comment.
    in_comment: bool,
    /// Retain heredoc delimiters declared on the current shell command line.
    pending_heredocs: VecDeque<HeredocDelimiter>,
    /// Retain the currently copied heredoc body, if one has begun.
    heredoc_body: Option<HeredocBody>,
}

/// Retain one heredoc delimiter and its tab-stripping rule.
struct HeredocDelimiter {
    /// Store the delimiter after POSIX quote removal.
    text: String,
    /// Record whether `<<-` strips leading tabs before delimiter matching.
    strips_leading_tabs: bool,
}

/// Retain state needed to recognise a terminating heredoc line.
struct HeredocBody {
    /// Retain the declaration that selected this body.
    delimiter: HeredocDelimiter,
    /// Record the first source character of the current heredoc line.
    line_start: usize,
}

/// Report that an escape would modify a marker lowered immediately after it.
///
/// The heredoc renderer is a pure string transformation, so it states this
/// condition with a private unit error that both recipe traversals convert into
/// the public interpolation diagnostic.
#[derive(Debug, Clone, Copy)]
pub(super) struct EscapeBeforeMarker;

/// Report whether the backslash at `pos` escapes a marker that follows it.
///
/// Callers only ask about a backslash they have not already consumed as an
/// escaped pair. A marker is a hazard only when the insertion site decides what
/// the backslash escapes, so single quotes and markers that end outside
/// `bound` are not rejected here.
pub(super) fn backslash_escapes_marker(
    chars: &[char],
    pos: usize,
    is_single_quoted: bool,
    bound: Option<usize>,
) -> bool {
    if is_single_quoted || chars.get(pos) != Some(&'\\') {
        return false;
    }
    let Some((_, skip)) = find_substitution(chars, pos + 1) else {
        return false;
    };
    bound.is_none_or(|end| pos + 1 + skip <= end)
}

/// Group the current source character with the output it must preserve.
pub(super) struct PosixCharacter<'chars, 'output> {
    /// Borrow the source-character buffer for lexical checks.
    pub(super) chars: &'chars [char],
    /// Record the source position of the current character.
    pub(super) pos: usize,
    /// Store the source character being processed.
    pub(super) ch: char,
    /// Borrow the output being built by the interpolation traversal.
    pub(super) output: &'output mut String,
}

impl PosixLexicalState {
    /// Initialise empty lexical state for one interpolation traversal.
    pub(super) const fn new() -> Self {
        Self {
            in_comment: false,
            pending_heredocs: VecDeque::new(),
            heredoc_body: None,
        }
    }

    /// Preserve one character from a comment or heredoc body when active.
    pub(super) fn append_inert_character(
        &mut self,
        character: &mut PosixCharacter<'_, '_>,
    ) -> bool {
        if self.in_comment {
            character.output.push(character.ch);
            if character.ch == '\n' {
                self.in_comment = false;
                self.begin_next_heredoc(character.pos + 1);
            }
            return true;
        }
        let Some(body) = self.heredoc_body.as_mut() else {
            return false;
        };
        character.output.push(character.ch);
        if character.ch == '\n' {
            if body.matches_terminator(character.chars, character.pos) {
                self.heredoc_body = None;
                self.begin_next_heredoc(character.pos + 1);
            } else {
                body.line_start = character.pos + 1;
            }
        }
        true
    }

    /// Begin a POSIX comment after its opening number sign is copied.
    pub(super) const fn begin_comment(&mut self) {
        self.in_comment = true;
    }

    /// Report whether an unquoted number sign begins a POSIX comment.
    pub(super) fn starts_comment(chars: &[char], pos: usize, ch: char) -> bool {
        ch == '#' && is_shell_word_boundary(chars, pos)
    }

    /// Preserve one heredoc declaration and queue its body delimiter.
    ///
    /// # Errors
    ///
    /// Returns [`EscapeBeforeMarker`] when the delimiter word would lower a
    /// marker directly after an active escape.
    pub(super) fn append_heredoc_declaration(
        &mut self,
        character: &mut PosixCharacter<'_, '_>,
        bindings: &CommandBindings,
    ) -> Result<Option<usize>, EscapeBeforeMarker> {
        if character.ch != '<' || character.chars.get(character.pos + 1) != Some(&'<') {
            return Ok(None);
        }
        let mut delimiter_start = character.pos + 2;
        let strips_leading_tabs = character.chars.get(delimiter_start) == Some(&'-');
        if strips_leading_tabs {
            delimiter_start += 1;
        }
        while matches!(character.chars.get(delimiter_start), Some(' ' | '\t')) {
            delimiter_start += 1;
        }
        let Some((_, end)) = parse_delimiter(character.chars, delimiter_start) else {
            return Ok(None);
        };
        let rendered_delimiter = render_delimiter(character.chars, delimiter_start, end, bindings)?;
        let rendered_chars: Vec<_> = rendered_delimiter.chars().collect();
        let Some((text, _)) = parse_delimiter(&rendered_chars, 0) else {
            return Ok(None);
        };
        let Some(declaration) = character.chars.get(character.pos..delimiter_start) else {
            return Ok(None);
        };
        character.output.extend(declaration.iter());
        character.output.push_str(&rendered_delimiter);
        self.pending_heredocs.push_back(HeredocDelimiter {
            text,
            strips_leading_tabs,
        });
        Ok(Some(end))
    }

    /// Begin the first queued heredoc body after its declaration line ends.
    pub(super) fn begin_pending_heredoc_after_newline(&mut self, next_pos: usize) {
        self.begin_next_heredoc(next_pos);
    }

    /// Begin the next queued heredoc body, if its declaration has completed.
    fn begin_next_heredoc(&mut self, line_start: usize) {
        let Some(delimiter) = self.pending_heredocs.pop_front() else {
            return;
        };
        self.heredoc_body = Some(HeredocBody {
            delimiter,
            line_start,
        });
    }
}

impl HeredocBody {
    /// Report whether the current source line terminates this heredoc body.
    fn matches_terminator(&self, chars: &[char], line_end: usize) -> bool {
        let Some(line) = chars.get(self.line_start..line_end) else {
            return false;
        };
        if self.delimiter.strips_leading_tabs {
            return line
                .iter()
                .skip_while(|character| **character == '\t')
                .copied()
                .eq(self.delimiter.text.chars());
        }
        line.iter().copied().eq(self.delimiter.text.chars())
    }
}

/// Report whether `pos` follows a boundary where POSIX permits a comment.
fn is_shell_word_boundary(chars: &[char], pos: usize) -> bool {
    let Some(previous_pos) = preceding_shell_character(chars, pos) else {
        return true;
    };
    is_unescaped_character(chars, pos)
        && is_unescaped_character(chars, previous_pos)
        && chars.get(previous_pos).is_some_and(|previous| {
            previous.is_whitespace() || matches!(previous, ';' | '|' | '&' | '(' | ')' | '<' | '>')
        })
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
fn is_unescaped_character(chars: &[char], pos: usize) -> bool {
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
