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

/// Describe one source character offered to a substitution's `case` tracker.
///
/// The four values are one observation rather than independent arguments: the
/// character, what it escapes, whether it is word data, and the parenthesis
/// depth a construct spelled here would open at all describe the same position.
#[derive(Clone, Copy)]
pub(super) struct CaseObservation {
    /// The character at the current position.
    ch: char,
    /// The character an unquoted backslash escapes.
    next: Option<char>,
    /// Whether the character is unquoted word data.
    is_unquoted: bool,
    /// The parenthesis depth of the active command substitution.
    parenthesis_depth: usize,
}

impl CaseObservation {
    /// Describe the character at one position of the active substitution.
    pub(super) const fn new(
        ch: char,
        next: Option<char>,
        is_unquoted: bool,
        parenthesis_depth: usize,
    ) -> Self {
        Self {
            ch,
            next,
            is_unquoted,
            parenthesis_depth,
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
    /// Record the parenthesis depth at which each open `case` began.
    ///
    /// A pattern terminator stands at the depth of the group that contains its
    /// `case`, which need not be the substitution's own outermost level, so the
    /// depth has to travel with the construct rather than be assumed.
    depths: Vec<usize>,
    /// Accumulate the current unquoted word so reserved words are recognised.
    word: String,
    /// Record that the traversal is inside an unquoted `#` comment.
    in_comment: bool,
    /// Record whether the next unquoted word would stand in command position.
    expects_command: bool,
    /// Record that a `case` keyword was seen, even if `esac` later closed it.
    saw_case: bool,
}

impl CaseNesting {
    /// Initialise an empty tracker for one command substitution.
    pub(super) const fn new() -> Self {
        Self {
            depths: Vec::new(),
            word: String::new(),
            in_comment: false,
            expects_command: true,
            saw_case: false,
        }
    }

    /// Record that the traversal began an unquoted comment.
    ///
    /// The traversal owns comment detection, so it reports the transition
    /// instead of letting the tracker infer it. Inferring from a `#` would put
    /// the tracker in comment mode for a `#` the traversal treats as word data,
    /// which is exactly the disagreement that mislocates the substitution
    /// closer.
    pub(super) fn begin_comment(&mut self) {
        self.in_comment = true;
        self.word.clear();
    }

    /// Record that the traversal consumed the newline that ended a comment.
    ///
    /// Comment text is copied verbatim and never offered to the tracker, so the
    /// terminating newline would otherwise be invisible to it. Missing that
    /// newline keeps the tracker inside the comment for the rest of the
    /// substitution, where it then ignores a later `case` and misreads the
    /// closer. A newline also starts a new command, so the next word stands in
    /// command position.
    pub(super) fn end_comment(&mut self) {
        self.in_comment = false;
        self.word.clear();
        self.expects_command = true;
    }

    /// Report whether the innermost open `case` began at `parenthesis_depth`.
    ///
    /// Only a `)` at the depth of its own construct terminates that pattern: a
    /// `)` one level deeper closes a group inside the pattern, and one level
    /// shallower closes the group that contains the whole `case`.
    pub(super) fn is_open_at(&self, parenthesis_depth: usize) -> bool {
        self.depths.last() == Some(&parenthesis_depth)
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
    ///
    /// The observation carries the depth a construct would open at, so the
    /// pattern terminator can later be matched against it.
    pub(super) fn observe(&mut self, observation: CaseObservation) {
        if !observation.is_unquoted {
            self.finish_word(observation.parenthesis_depth);
            // Quoted text is part of a word, so whatever follows it is not
            // in command position.
            self.expects_command = false;
        } else if self.in_comment {
            self.observe_comment_character(observation.ch);
        } else {
            self.observe_word_character(observation);
        }
    }

    /// End a comment when the line does, and ignore the rest of its text.
    const fn observe_comment_character(&mut self, ch: char) {
        if ch == '\n' {
            self.in_comment = false;
            self.expects_command = true;
        }
    }

    /// Accumulate one character of unquoted word data.
    fn observe_word_character(&mut self, observation: CaseObservation) {
        if observation.ch == '\\' {
            self.word
                .extend(observation.next.filter(|escaped| *escaped != '\n'));
            return;
        }
        if ends_word(observation.ch) {
            self.finish_word(observation.parenthesis_depth);
            if !is_blank(observation.ch) {
                self.expects_command = starts_command_after(observation.ch);
            }
            return;
        }
        self.word.push(observation.ch);
    }

    /// Apply the completed word to the `case` nesting state.
    ///
    /// A reserved word counts only in command position, so `printf esac` passes
    /// `esac` as an argument and leaves the construct open. Popping there would
    /// end the construct early, and the pattern terminator that follows would
    /// then be read as the substitution closer one `)` too soon.
    fn finish_word(&mut self, parenthesis_depth: usize) {
        if self.word.is_empty() {
            return;
        }
        let is_command_position = self.expects_command;
        match self.word.as_str() {
            "case" if is_command_position => {
                self.depths.push(parenthesis_depth);
                self.saw_case = true;
                self.expects_command = false;
            }
            // A case pattern may itself be spelled `esac`. Only a construct
            // open at this depth is the one this word closes; anything deeper
            // is inside a pattern group and must not close it.
            "esac" if is_command_position && self.is_open_at(parenthesis_depth) => {
                self.depths.pop();
                self.expects_command = false;
            }
            word => self.expects_command = is_command_position && starts_command_list(word),
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

/// Report whether `ch` is an unquoted blank that leaves token position alone.
const fn is_blank(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

/// Report whether a command may begin immediately after the operator `ch`.
///
/// A redirection operator introduces a filename rather than a command, so it is
/// deliberately absent: `esac` after `>` is a filename, not a keyword.
const fn starts_command_after(ch: char) -> bool {
    matches!(ch, ';' | '&' | '|' | '\n' | '(' | ')')
}

/// Report whether a reserved word ends with the next word in command position.
///
/// These words open or continue a command list, so the following word is again
/// a command word. Any other word completes an argument list.
fn starts_command_list(word: &str) -> bool {
    matches!(
        word,
        "if" | "then" | "else" | "elif" | "while" | "until" | "do" | "!" | "time"
    )
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
