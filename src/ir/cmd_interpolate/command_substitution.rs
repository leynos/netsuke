//! Retain local state for one POSIX-style command substitution.
//!
//! Command substitutions have independent quote and parenthesis state, which
//! prevents their contents from changing the context of surrounding recipe
//! markers. This module also owns the two boundary checks that stop a `case`
//! pattern terminator from passing for the substitution closer.

use super::case_nesting::CaseNesting;
use super::find_substitution;
use super::posix_lexical::{SourceChar, SubstitutionKind, is_unescaped_character};
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
    /// Record what the delimiter opened: a command list or an arithmetic body.
    kind: SubstitutionKind,
    /// Record the `$` that opened this substitution for source-range checks.
    pub(super) start: usize,
    /// Track `case ... esac` nesting so a pattern terminator is not a closer.
    pub(super) case_nesting: CaseNesting,
}

impl CommandSubstitution {
    /// Initialise the state required after a `$(` delimiter.
    ///
    /// `start` is the `$` that opened the substitution, kept for source-range
    /// checks. `kind` records whether the delimiter was `$((`, so a caller can
    /// keep arithmetic operators such as `<<` away from shell heredoc tracking.
    /// Arithmetic bodies expand numbers, so they never hold a heredoc
    /// declaration.
    pub(super) const fn new(start: usize, kind: SubstitutionKind) -> Self {
        Self {
            quote_context: QuoteContext::Unquoted,
            parenthesis_depth: 1,
            kind,
            start,
            case_nesting: CaseNesting::new(),
        }
    }

    /// Report whether this substitution's body is an arithmetic expansion.
    pub(super) const fn is_arithmetic(&self) -> bool {
        matches!(self.kind, SubstitutionKind::Arithmetic)
    }
}

/// Describe the `)` that closed a command substitution.
pub(super) struct SubstitutionBoundary {
    /// Position of the closing `)`.
    pub(super) pos: usize,
    /// The `$` that opened the substitution, when this `)` closed it.
    pub(super) start: Option<usize>,
    /// Whether the substitution's own tracker saw a `case` keyword.
    pub(super) saw_case: bool,
    /// Whether the tracker saw a `case` word outside its modelled positions.
    pub(super) has_unplaced_case: bool,
}

impl SubstitutionBoundary {
    /// Record the `)` at `pos`, the substitution opener it closed, and what the
    /// tracker saw of `case` keywords.
    pub(super) const fn new(
        pos: usize,
        start: Option<usize>,
        saw_case: bool,
        has_unplaced_case: bool,
    ) -> Self {
        Self {
            pos,
            start,
            saw_case,
            has_unplaced_case,
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

/// Reject a closer whose `case` keyword the traversal could not place.
///
/// Bash keeps a command position open after `time` options and after a
/// `coproc` name. The traversal models the word forms it can read, but a
/// quoted or expanded word in either slot is opaque, so a `case` behind one
/// looks like argument data. If that `case` begins a real construct, its
/// pattern terminator would pass for the substitution closer and a later
/// marker would be encoded for the outer quote context.
///
/// The check is deliberately conservative. It fires only when a whole-word
/// `case` also has a whole-word `in` before the closer, and only when a marker
/// follows the closer: with no later marker the substitution's text is copied
/// unchanged, so the misread boundary cannot alter the output.
///
/// # Errors
///
/// Returns [`IrGenError::InvalidCommand`] when a `case` construct may begin at
/// a position the traversal read as argument data.
pub(super) fn reject_unplaced_case(
    chars: &[char],
    boundary: &SubstitutionBoundary,
) -> Result<(), IrGenError> {
    let Some(start) = boundary.start else {
        return Ok(());
    };
    let is_unplaceable_case = boundary.has_unplaced_case
        && contains_case_in_prefix(chars, start, boundary.pos)
        && has_marker_after(chars, boundary.pos);
    if is_unplaceable_case {
        return Err(unplaced_case_error());
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

/// Build the diagnostic for a `case` the traversal could not place.
///
/// The recipe is left out for the same reason as the ambiguity diagnostic, and
/// so is a position: the condition is about the whole substitution rather than
/// one character. The explanation is its own Fluent message rather than a
/// snippet argument, so a non-English locale does not render an English
/// sentence inside a translated diagnostic.
const fn unplaced_case_error() -> IrGenError {
    let message = localization::message(keys::IR_UNPLACED_CASE);
    IrGenError::InvalidCommand {
        command: String::new(),
        snippet: String::new(),
        message,
    }
}

/// Report whether a Netsuke marker appears after `pos`.
///
/// Only a marker that follows the closer can observe the quote context this
/// boundary sets, so a misread boundary with no later marker changes no output
/// and is left alone.
fn has_marker_after(chars: &[char], pos: usize) -> bool {
    (pos + 1..chars.len()).any(|index| find_substitution(chars, index).is_some())
}

/// Report whether a whole-word `case` is followed by a whole-word `in`.
///
/// `in` is required by the shell before the first pattern terminator, so a
/// `case` without one cannot be the construct whose `)` ends the pattern.
/// Matching it keeps ordinary argument data such as `echo case` from failing
/// closed, while `grep case in file` still over-approximates.
///
/// One forward pass answers the whole question: the first keyword that matters
/// is the earliest `case`, and any later `in` completes its header. Rescanning
/// the suffix from every `case` would make a long run of `case` arguments
/// quadratic before the recipe is even accepted.
fn contains_case_in_prefix(chars: &[char], start: usize, end: usize) -> bool {
    let mut saw_case = false;
    for pos in start..end {
        if !saw_case && is_keyword_at(chars, pos, &CASE_KEYWORD) {
            saw_case = true;
            continue;
        }
        if saw_case && is_keyword_at(chars, pos, &IN_KEYWORD) {
            return true;
        }
    }
    false
}

/// Report whether `chars[start..end]` contains the `case` reserved word.
///
/// The scan is deliberately lexical: it does not skip quotes, comments, or
/// nested substitutions, so it can report a `case` that is not a keyword. That
/// over-approximation is safe because the caller fails closed on any match
/// rather than guessing which `)` closes the substitution.
pub(super) fn contains_case_keyword(chars: &[char], start: usize, end: usize) -> bool {
    contains_keyword(chars, start, end, &CASE_KEYWORD)
}

/// Spell the `case` reserved word.
const CASE_KEYWORD: [char; 4] = ['c', 'a', 's', 'e'];

/// Spell the `in` reserved word that completes a `case` header.
const IN_KEYWORD: [char; 2] = ['i', 'n'];

/// Report whether `chars[start..end]` contains `keyword` as a whole word.
fn contains_keyword(chars: &[char], start: usize, end: usize, keyword: &[char]) -> bool {
    (start..end).any(|pos| is_keyword_at(chars, pos, keyword))
}

/// Report whether `keyword` appears at `pos` as a whole word.
///
/// Bash deletes a backslash-newline pair before it parses, so `c`, a
/// continuation, then `ase` spells `case`, and the same holds for `in`. The
/// scan therefore steps over those pairs inside the keyword and on both sides
/// of it, matching the word the shell would build rather than the characters
/// the file contains. A continuation that joins the keyword to a neighbouring
/// word is joined here too, so it does not count as a word of its own.
///
/// The candidate must spell the first character at its own position, so a
/// position inside a run of continuations is rejected in constant time instead
/// of walking the rest of the run. A keyword written after a continuation is
/// still found at the index of its first letter.
fn is_keyword_at(chars: &[char], pos: usize, keyword: &[char]) -> bool {
    if chars.get(pos) != keyword.first() {
        return false;
    }
    let mut cursor = pos;
    for expected in keyword {
        cursor = skip_line_continuations(chars, cursor);
        if chars.get(cursor) != Some(expected) {
            return false;
        }
        cursor += 1;
    }
    let starts_word = previous_effective_char(chars, pos).is_none_or(|index| {
        chars
            .get(index)
            .is_some_and(|ch| SourceChar::new(*ch).is_word_terminator())
    });
    let after = skip_line_continuations(chars, cursor);
    let ends_word = chars
        .get(after)
        .is_none_or(|ch| SourceChar::new(*ch).is_word_terminator());
    starts_word && ends_word
}

/// Return the first index at or after `pos` that no continuation hides.
///
/// An unescaped backslash immediately followed by a newline is deleted before
/// parsing, so the character the shell reads next is the one after the pair. A
/// backslash that is itself escaped starts no pair: in `\\<newline>` the
/// newline is an ordinary terminator.
fn skip_line_continuations(chars: &[char], mut pos: usize) -> usize {
    while is_line_continuation_at(chars, pos) {
        pos += 2;
    }
    pos
}

/// Report whether an unescaped backslash-newline pair starts at `pos`.
///
/// The pair is deleted before parsing, so `c` followed by a continuation and
/// `ase` still spells `case`. The test is one predicate so the scan above keeps
/// a single branch, and each of its own conditions stays within the branch
/// limit.
fn is_line_continuation_at(chars: &[char], pos: usize) -> bool {
    let is_pair = chars.get(pos) == Some(&'\\') && chars.get(pos + 1) == Some(&'\n');
    is_pair && is_unescaped_character(chars, pos)
}

/// Return the index of the character that really precedes `pos`.
///
/// Continuations are deleted before parsing, so a keyword preceded by one is
/// preceded by the character before the pair, which decides whether it starts a
/// word. Returning `None` means the position opens the source.
fn previous_effective_char(chars: &[char], pos: usize) -> Option<usize> {
    let mut end = pos;
    while ends_with_continuation(chars, end) {
        end -= 2;
    }
    end.checked_sub(1)
}

/// Report whether a backslash-continued newline ends at `end`.
///
/// The pair is deleted before parsing only when the backslash is unescaped, so
/// an index that follows one really follows the character before it. In
/// `\\<newline>` the first backslash quotes the second and the newline is an
/// ordinary separator, which is what lets the `in` after it start a word.
fn ends_with_continuation(chars: &[char], end: usize) -> bool {
    let Some(backslash) = end.checked_sub(2) else {
        return false;
    };
    chars.get(end - 1) == Some(&'\n')
        && chars.get(backslash) == Some(&'\\')
        && is_unescaped_character(chars, backslash)
}
