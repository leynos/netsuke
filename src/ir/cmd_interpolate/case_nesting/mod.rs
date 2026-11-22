//! Track `case ... esac` nesting inside one command substitution.
//!
//! A `)` that terminates a `case` pattern is an operator, while the `)` that
//! closes `$()` stays part of the surrounding word. The two are lexically
//! identical without the shell grammar, so this tracker counts the reserved
//! words that stand in command position and reports the depth at which each
//! pattern terminator belongs. Bash keeps a command position open after `time`
//! options and a `coproc` name, so the tracker models those word forms too;
//! a quoted or expanded word in either slot stays opaque, and
//! [`CaseNesting::has_unplaced_case`] reports the keyword it could not place.

mod lexical;

mod scan;

mod state;

use scan::TokenSpelling;

use state::{CaseFrame, CasePhase, CaseSightings, PrefixSlot};

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

    /// Report whether the shell reads this character before the next one.
    ///
    /// An unquoted backslash-newline pair is deleted before tokenizing, so the
    /// character after the pair follows whatever preceded the backslash: `;`,
    /// a continuation, then `;` spells the `;;` operator. Every other
    /// character, including an escaped real character such as `;` in `\;`, is
    /// read by the shell and does precede the next one.
    const fn is_read_before_next(&self) -> bool {
        let is_continuation = matches!((self.ch, self.next), ('\\', Some('\n')));
        !(is_continuation && self.is_unquoted)
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
pub(super) struct CaseNesting {
    /// Record each open `case` construct and the part of it being read.
    ///
    /// A pattern terminator stands at the depth of the group that contains its
    /// `case`, which need not be the substitution's own outermost level, so the
    /// depth has to travel with the construct rather than be assumed. The phase
    /// travels with it too, because `esac` is a keyword at the start of a
    /// pattern list but pattern text after `|`.
    frames: Vec<CaseFrame>,
    /// Accumulate the current unquoted word so reserved words are recognized.
    word: String,
    /// Record what the current token is: not started, bare, or quoted.
    ///
    /// Bash recognizes a reserved word only in its bare spelling, so a token
    /// holding a quote or an escape of a real character is ordinary data even
    /// when it spells `esac`. The state covers the whole token rather than the
    /// accumulated word, because a quote ends the word without ending the
    /// token: `esac""` and `e\sac` are one token each. Quoted characters never
    /// enter `word`, so the state also records that a wholly quoted token has
    /// begun when the buffer is still empty.
    spelling: TokenSpelling,
    /// Record that the traversal is inside an unquoted `#` comment.
    in_comment: bool,
    /// Record whether the next unquoted word would stand in command position.
    expects_command: bool,
    /// Record the character read immediately before the current one.
    ///
    /// Only its role in spotting a `;;` or `;&` matters: both are two-character
    /// operators, and the traversal offers one character at a time. A deleted
    /// continuation is not a character the shell reads, so it is never stored:
    /// `;`, a continuation, then `;` still spells `;;`.
    previous: Option<char>,
    /// Record which `case` keywords have been seen and where.
    sightings: CaseSightings,
    /// Record which compound-command prefix slot the next word may fill.
    prefix_slot: PrefixSlot,
}
impl CaseNesting {
    /// Initialize an empty tracker for one command substitution.
    pub(super) const fn new() -> Self {
        Self {
            frames: Vec::new(),
            word: String::new(),
            spelling: TokenSpelling::Bare,
            in_comment: false,
            expects_command: true,
            previous: None,
            sightings: CaseSightings::None,
            prefix_slot: PrefixSlot::None,
        }
    }

    /// Report whether any open `case` began at `parenthesis_depth`.
    ///
    /// Only a `)` at the depth of its own construct terminates that pattern: a
    /// `)` one level deeper closes a group inside the pattern, and one level
    /// shallower closes the group that contains the whole `case`. Every frame
    /// is consulted rather than only the innermost, because a construct at a
    /// deeper depth must not hide the one whose terminator this position could
    /// be.
    pub(super) fn is_open_at(&self, parenthesis_depth: usize) -> bool {
        self.frames
            .iter()
            .any(|frame| frame.depth == parenthesis_depth)
    }

    /// Consume the optional `(` before a pattern list, reporting whether this
    /// position is one.
    ///
    /// Bash accepts one optional `(` immediately before a pattern list. The
    /// traversal cannot tell it from a grouped expression without the
    /// construct's phase, so the tracker answers from the frame open at this
    /// depth and records that the list has started. Recording it is what keeps
    /// an `esac` written as the first pattern from closing the construct: the
    /// closer is valid only where a pattern list would start, and after the
    /// opener that position is pattern text. Only a list that has not started
    /// yet can begin with the opener: after a pattern word the `(` is a group,
    /// and after a `|` alternative Bash rejects one outright.
    pub(super) fn consume_pattern_opener(&mut self, parenthesis_depth: usize) -> bool {
        let Some(frame) = self.open_construct_at_mut(parenthesis_depth) else {
            return false;
        };
        let is_pattern_phase = matches!(frame.phase, CasePhase::Pattern);
        let is_at_list_start = !frame.pattern_started && !frame.after_alternative;
        let is_opener = is_pattern_phase && is_at_list_start;
        if is_opener {
            frame.pattern_started = true;
        }
        is_opener
    }

    /// Report whether a `case` keyword appeared anywhere in this substitution.
    ///
    /// The raw-source scan that backs the ambiguity guard cannot see a keyword
    /// spelled with escapes or line continuations, so the tracker reports its
    /// own sighting and the guard fails closed on either.
    pub(super) const fn saw_case(&self) -> bool {
        self.sightings.has_any()
    }

    /// Report whether a `case` word stood outside the modelled command positions.
    ///
    /// The boundary check reads this alongside a raw-source scan for a complete
    /// `case ... in` prefix: together they say a construct may begin where the
    /// tracker saw argument data, which is the one remaining way a pattern
    /// terminator could pass for the substitution closer.
    pub(super) const fn has_unplaced_case(&self) -> bool {
        self.sightings.has_unplaced()
    }
}
