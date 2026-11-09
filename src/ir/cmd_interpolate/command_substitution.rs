//! Retain local state for one POSIX-style command substitution.
//!
//! Command substitutions have independent quote and parenthesis state, which
//! prevents their contents from changing the context of surrounding recipe
//! markers.

use super::posix_lexical::is_word_terminator;
use super::{IrGenError, QuoteContext};
use crate::localization::{self, keys};

/// Identify one command-substitution delimiter recognised during traversal.
pub(super) enum CommandSubstitutionDelimiter {
    /// Begin a nested `$()` command substitution.
    Start,
    /// Open a grouped expression within the active command substitution.
    NestedOpen,
    /// Close a grouped expression or the active command substitution.
    Close,
}

/// Retain parsing state for one active command substitution.
pub(super) struct CommandSubstitution {
    /// Record the quote state local to this command-substitution body.
    pub(super) quote_context: QuoteContext,
    /// Count this substitution's opening parenthesis and nested grouped forms.
    pub(super) parenthesis_depth: usize,
    /// Record the `$` that opened this substitution for source-range checks.
    pub(super) start: usize,
    /// Track `case ... esac` nesting so a pattern terminator is not a closer.
    pub(super) case_nesting: CaseNesting,
}

impl CommandSubstitution {
    /// Initialise the state required after a `$(` delimiter.
    pub(super) const fn new(start: usize) -> Self {
        Self {
            quote_context: QuoteContext::Unquoted,
            parenthesis_depth: 1,
            start,
            case_nesting: CaseNesting::new(),
        }
    }
}

/// Track `case ... esac` nesting inside one command substitution.
///
/// A `)` that terminates a `case` pattern is an operator, while the `)` that
/// closes `$()` stays part of the surrounding word. The two are lexically
/// identical, so the traversal counts the reserved words and keeps the
/// substitution open across pattern terminators. That makes the real closer the
/// one that is checked for an ambiguous `#`, instead of the pattern terminator
/// that merely happens to come first.
#[derive(Default)]
pub(super) struct CaseNesting {
    /// Count `case` keywords not yet matched by `esac`.
    depth: usize,
    /// Accumulate the current unquoted word so reserved words are recognised.
    word: String,
    /// Record that the traversal is inside an unquoted `#` comment.
    in_comment: bool,
    /// Record that a `case` keyword was seen, even if `esac` later closed it.
    saw_case: bool,
}

impl CaseNesting {
    /// Initialise an empty tracker for one command substitution.
    pub(super) const fn new() -> Self {
        Self {
            depth: 0,
            word: String::new(),
            in_comment: false,
            saw_case: false,
        }
    }

    /// Report whether a `case` construct is open inside this substitution.
    pub(super) const fn is_open(&self) -> bool {
        self.depth > 0
    }

    /// Report whether a `case` keyword appeared anywhere in this substitution.
    ///
    /// The raw-source scan that backs the ambiguity guard cannot see a keyword
    /// spelled with escapes or line continuations, so the tracker reports its
    /// own sighting and the guard fails closed on either.
    pub(super) const fn saw_case(&self) -> bool {
        self.saw_case
    }

    /// Record one source character and update the reserved-word count.
    ///
    /// Only unquoted word data can spell a reserved word, so a quote or a
    /// separator ends the word being accumulated. Both traversals consume an
    /// escaped character without reporting it separately, so a backslash reads
    /// its own escape from `next`: `ca\se` and `c\<newline>ase` both spell
    /// `case`, while a line continuation removes both characters. A `case`
    /// inside a quoted string or a comment therefore never opens a construct.
    pub(super) fn observe(&mut self, ch: char, next: Option<char>, is_unquoted: bool) {
        if !is_unquoted {
            self.finish_word();
        } else if self.in_comment {
            self.observe_comment_character(ch);
        } else {
            self.observe_word_character(ch, next);
        }
    }

    /// End a comment when the line does, and ignore the rest of its text.
    const fn observe_comment_character(&mut self, ch: char) {
        if ch == '\n' {
            self.in_comment = false;
        }
    }

    /// Accumulate one character of unquoted word data.
    fn observe_word_character(&mut self, ch: char, next: Option<char>) {
        if ch == '\\' {
            self.word.extend(next.filter(|escaped| *escaped != '\n'));
            return;
        }
        if ends_word(ch) {
            self.finish_word();
            return;
        }
        // A number sign starts a comment only at a word start; mid-word it is
        // ordinary word data, so `case#x` must not read as the keyword.
        if ch == '#' && self.word.is_empty() {
            self.in_comment = true;
            return;
        }
        self.word.push(ch);
    }

    /// Apply the completed word to the `case` nesting count.
    fn finish_word(&mut self) {
        match self.word.as_str() {
            "case" => {
                self.depth += 1;
                self.saw_case = true;
            }
            "esac" => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        self.word.clear();
    }
}

/// Report whether `ch` ends the unquoted word being accumulated.
///
/// A quote character ends the word as surely as a separator does: the quoted
/// text that follows is not word data, so a reserved word cannot continue into
/// it. Both categories are kept in one predicate because the caller treats them
/// identically.
const fn ends_word(ch: char) -> bool {
    is_word_terminator(ch) || matches!(ch, '\'' | '"' | '`')
}

/// Describe the `)` that closed a command substitution.
pub(super) struct SubstitutionBoundary {
    /// Position of the closing `)`.
    pub(super) pos: usize,
    /// The `$` that opened the substitution, when this `)` closed it.
    pub(super) start: Option<usize>,
    /// Whether the substitution's own tracker saw a `case` keyword.
    pub(super) saw_case: bool,
}

impl SubstitutionBoundary {
    /// Record the `)` at `pos`, the substitution opener it closed, and whether
    /// that substitution contained a `case` keyword.
    pub(super) const fn new(pos: usize, start: Option<usize>, saw_case: bool) -> Self {
        Self {
            pos,
            start,
            saw_case,
        }
    }
}

/// Reject a `)#` boundary whose `)` may terminate a `case` pattern.
///
/// A `)` that closes `$()` is part of the surrounding word, so `#` after it is
/// word data. A `)` that terminates a `case` pattern is an operator, so `#`
/// after it starts a comment. The two are lexically identical without the
/// `case` grammar, so a substitution containing `case` fails closed rather than
/// guessing and encoding a later marker for the wrong context.
///
/// The enclosing quote context is deliberately not consulted. A substitution
/// is parsed as its own script, so outer double quotes do not extend into it
/// and cannot make the boundary unambiguous. Callers that model a shell
/// without `case` patterns decide for themselves whether to call this.
///
/// # Errors
///
/// Returns [`IrGenError::AmbiguousSubstitutionBoundary`] when the boundary is
/// ambiguous.
pub(super) fn reject_ambiguous_case_comment(
    chars: &[char],
    boundary: &SubstitutionBoundary,
) -> Result<(), IrGenError> {
    let Some(start) = boundary.start else {
        return Ok(());
    };
    let is_comment_boundary = chars.get(boundary.pos + 1) == Some(&'#');
    let has_case = boundary.saw_case || contains_case_keyword(chars, start, boundary.pos);
    if is_comment_boundary && has_case {
        return Err(ambiguous_boundary_error(boundary.pos + 1));
    }
    Ok(())
}

/// Build the diagnostic for an ambiguous `)#` boundary.
///
/// The recipe is deliberately left out. The rejected text is exactly the text
/// the scanner could not classify, so quoting it would copy executable shell
/// syntax into a log line; the character offset is enough to find the boundary
/// in the source, and it survives the 160-character truncation that the
/// invalid-command diagnostic applies.
fn ambiguous_boundary_error(position: usize) -> IrGenError {
    let message = localization::message(keys::IR_AMBIGUOUS_SUBSTITUTION_BOUNDARY)
        .with_arg("position", position);
    IrGenError::AmbiguousSubstitutionBoundary { position, message }
}

/// Report whether `chars[start..end]` contains the `case` reserved word.
///
/// The scan is deliberately lexical: it does not skip quotes, comments, or
/// nested substitutions, so it can report a `case` that is not a keyword. That
/// over-approximation is safe because the caller fails closed on any match
/// rather than guessing which `)` closes the substitution.
pub(super) fn contains_case_keyword(chars: &[char], start: usize, end: usize) -> bool {
    (start..end).any(|pos| is_case_keyword_at(chars, pos))
}

/// Report whether the four characters at `pos` spell `case` as a whole word.
fn is_case_keyword_at(chars: &[char], pos: usize) -> bool {
    const KEYWORD: [char; 4] = ['c', 'a', 's', 'e'];
    if chars.get(pos..pos + KEYWORD.len()) != Some(KEYWORD.as_slice()) {
        return false;
    }
    let starts_word = pos == 0 || chars.get(pos - 1).is_some_and(|ch| is_word_terminator(*ch));
    let ends_word = chars
        .get(pos + KEYWORD.len())
        .is_none_or(|ch| is_word_terminator(*ch));
    starts_word && ends_word
}
