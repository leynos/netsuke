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
mod state;

use lexical::{ends_word, is_blank, is_quote, starts_command_after};
use state::{CaseFrame, CasePhase, CaseSightings, PrefixSlot, WordRole};

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
    /// Record whether the current token is spelled without quoting or escapes.
    ///
    /// Bash recognizes a reserved word only in its bare spelling, so a token
    /// holding a quote or an escape of a real character is ordinary data even
    /// when it spells `esac`. The flag covers the whole token rather than the
    /// accumulated word, because a quote ends the word without ending the
    /// token: `esac""` and `e\sac` are one token each.
    word_is_literal: bool,
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
            word_is_literal: true,
            in_comment: false,
            expects_command: true,
            previous: None,
            sightings: CaseSightings::None,
            prefix_slot: PrefixSlot::None,
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
        self.word_is_literal = true;
        self.expects_command = true;
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
            // Quoting is part of the token even where it ends the word, so the
            // fragment completed here is not a reserved word.
            self.word_is_literal = false;
            self.finish_word(observation.parenthesis_depth);
            // Quoted text is part of a word, so whatever follows it is not
            // in command position.
            self.expects_command = false;
        } else if self.in_comment {
            self.observe_comment_character(observation.ch);
        } else {
            self.observe_word_character(observation);
        }
        if observation.is_read_before_next() {
            self.previous = Some(observation.ch);
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
            self.fold_escape(observation.next);
            return;
        }
        if ends_word(observation.ch) {
            self.finish_word_at_boundary(observation.ch, observation.parenthesis_depth);
            self.apply_operator(observation.ch, observation.parenthesis_depth);
            if !is_blank(observation.ch) {
                self.expects_command = starts_command_after(observation.ch);
            }
            return;
        }
        self.word.push(observation.ch);
    }

    /// Record an escape in the word being accumulated.
    ///
    /// Bash deletes a backslash-newline pair before tokenizing, so a keyword
    /// written across a continuation is still that keyword: `c\<newline>ase`
    /// spells `case` with nothing quoted. Any other escape is folded into the
    /// word instead, where `e\sac` spells `esac` but is ordinary data because
    /// the shell never reads it as a reserved word.
    fn fold_escape(&mut self, escaped: Option<char>) {
        if escaped != Some('\n') {
            self.word_is_literal = false;
        }
        self.word.extend(escaped.filter(|next| *next != '\n'));
    }

    /// Finish the word ending at `boundary` and set the spelling that follows.
    ///
    /// A quote ends the fragment being accumulated but belongs to the same
    /// token, so it must mark the word non-literal before classification and
    /// leave the token non-literal afterwards. A blank or operator ends the
    /// token, so the word keeps the spelling it is classified with and the next
    /// word starts bare.
    fn finish_word_at_boundary(&mut self, boundary: char, parenthesis_depth: usize) {
        let starts_word = !is_quote(boundary);
        // Only the quote that ends this fragment is part of the token judged.
        self.word_is_literal &= starts_word;
        self.finish_word(parenthesis_depth);
        self.word_is_literal = starts_word;
    }

    /// Move the construct that contains this position across an operator.
    ///
    /// Only a construct open at this depth owns the operator: a `)` one level
    /// deeper closes a group inside the pattern, and a `;;` inside a subshell
    /// belongs to that subshell rather than to the outer arm.
    fn apply_operator(&mut self, ch: char, parenthesis_depth: usize) {
        let previous = self.previous;
        if let Some(frame) = self.open_construct_at_mut(parenthesis_depth) {
            frame.apply_operator(ch, previous);
        }
    }

    /// Return the innermost open construct that began at `parenthesis_depth`.
    ///
    /// A construct at a deeper depth is skipped rather than assumed: the
    /// operator belongs to the construct whose pattern list or arm contains
    /// this position, not to whichever frame happens to be on top.
    fn open_construct_at_mut(&mut self, parenthesis_depth: usize) -> Option<&mut CaseFrame> {
        self.frames
            .iter_mut()
            .rev()
            .find(|frame| frame.depth == parenthesis_depth)
    }

    /// Apply the completed word to the `case` nesting state.
    ///
    /// Classification reads the state before any of it is changed, so the word
    /// is judged against the position it was written in, and application then
    /// moves exactly the state that role owns.
    fn finish_word(&mut self, parenthesis_depth: usize) {
        if self.word.is_empty() {
            return;
        }
        let role = self.classify_word(parenthesis_depth);
        self.prefix_slot = PrefixSlot::None;
        self.apply_role(role, parenthesis_depth);
        self.word.clear();
    }

    /// Classify the completed word by its effect on the tracker.
    ///
    /// A reserved word counts only in command position, so `printf esac` passes
    /// `esac` as an argument and leaves the construct open. Popping there would
    /// end the construct early, and the pattern terminator that follows would
    /// then be read as the substitution closer one `)` too soon.
    ///
    /// `time` and `coproc` are tested before the prefix slots because both keep
    /// a command position open: the `case` that follows them is a reserved word
    /// even though it is not the first word of the command.
    fn classify_word(&self, parenthesis_depth: usize) -> WordRole {
        self.classify_within_construct(parenthesis_depth)
            .unwrap_or_else(|| self.classify_command_word(parenthesis_depth))
    }

    /// Classify a word read inside the innermost construct at this depth.
    ///
    /// Returns `None` where no construct covers the position, so the word is
    /// classified by the command positions that apply everywhere else. The
    /// distinction matters because a pattern list is the one place where a
    /// reserved word is data: `case x in esac)` closes the construct, while
    /// `case x in x|esac)` has `esac` as an alternative pattern. The header's
    /// first word is the subject whatever it spells, so `case in in (in) …`
    /// reads its second `in` as the keyword rather than as the subject.
    fn classify_within_construct(&self, parenthesis_depth: usize) -> Option<WordRole> {
        let frame = self
            .frames
            .iter()
            .rev()
            .find(|frame| frame.depth <= parenthesis_depth)?;
        let is_at_own_depth = frame.depth == parenthesis_depth;
        let word = self.word.as_str();
        match frame.phase {
            CasePhase::Header if is_at_own_depth && !frame.subject_seen => {
                Some(WordRole::CaseSubject)
            }
            CasePhase::Header if is_at_own_depth && word == "in" => Some(WordRole::EnterPattern),
            CasePhase::Pattern
                if frame.is_closed_by(parenthesis_depth, word, self.word_is_literal) =>
            {
                Some(WordRole::CloseCase)
            }
            // Every other word in the header or a pattern list is data: the
            // subject, an alternative pattern, or the text that follows `in`.
            CasePhase::Header | CasePhase::Pattern => Some(WordRole::Other {
                continues_command_list: false,
            }),
            CasePhase::Arm => None,
        }
    }

    /// Close the construct that began at `parenthesis_depth`.
    ///
    /// The innermost such construct is the one an `esac` terminates, which is
    /// also the only one classification reports as closing. Removing it by
    /// depth rather than from the top of the stack keeps a construct at a
    /// deeper depth from being closed in its place.
    fn close_construct_at(&mut self, parenthesis_depth: usize) {
        if let Some(index) = self
            .frames
            .iter()
            .rposition(|frame| frame.depth == parenthesis_depth)
        {
            self.frames.remove(index);
        }
    }
}
