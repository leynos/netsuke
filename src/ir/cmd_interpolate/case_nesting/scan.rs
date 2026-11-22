//! Scan one substitution's characters into words, comments, and operators.
//!
//! The traversal offers one character at a time, and this module turns that
//! stream into the token transitions the tracker's state machine consumes. It
//! owns the word buffer, comment handling, and operator application, leaving
//! the tracker module to the state values and the questions callers ask about
//! them.

use super::lexical::{ends_word, is_blank, is_quote, starts_command_after};
use super::state::{CaseFrame, CasePhase, PrefixSlot, WordRole};
use super::{CaseNesting, CaseObservation};

impl CaseNesting {
    /// Record that the traversal began an unquoted comment.
    ///
    /// The traversal owns comment detection, so it reports the transition
    /// instead of letting the tracker infer it. Inferring from a `#` would put
    /// the tracker in comment mode for a `#` the traversal treats as word data,
    /// which is exactly the disagreement that mislocates the substitution
    /// closer.
    pub(in crate::ir::cmd_interpolate) fn begin_comment(&mut self) {
        self.in_comment = true;
        self.word.clear();
        self.spelling = TokenSpelling::NotStarted;
    }

    /// Record that the traversal consumed the newline that ended a comment.
    ///
    /// Comment text is copied verbatim and never offered to the tracker, so the
    /// terminating newline would otherwise be invisible to it. Missing that
    /// newline keeps the tracker inside the comment for the rest of the
    /// substitution, where it then ignores a later `case` and misreads the
    /// closer. A newline also starts a new command, so the next word stands in
    /// command position.
    pub(in crate::ir::cmd_interpolate) fn end_comment(&mut self) {
        self.in_comment = false;
        self.word.clear();
        self.spelling = TokenSpelling::Bare;
        self.expects_command = true;
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
    pub(in crate::ir::cmd_interpolate) fn observe(&mut self, observation: CaseObservation) {
        if !observation.is_unquoted {
            // Quoting is part of the token even where it ends the word, so the
            // fragment completed here is not a reserved word. The quote also
            // begins a token that may leave the word buffer empty.
            self.spelling = TokenSpelling::Quoted;
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
            self.spelling = TokenSpelling::Quoted;
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
        if is_quote(boundary) {
            // A quote ends the fragment but not the token, so the word is
            // judged only when the token itself ends. Classifying here would
            // read the `in` of a subject such as `"x"in` as the keyword.
            self.spelling = TokenSpelling::Quoted;
            return;
        }
        let had_word = !self.word.is_empty();
        let token_had_quotes = matches!(self.spelling, TokenSpelling::Quoted);
        self.finish_word(parenthesis_depth);
        if !had_word && token_had_quotes {
            self.record_quoted_token(parenthesis_depth);
        }
        // A blank or operator ends the token, so the next word starts bare.
        self.spelling = TokenSpelling::Bare;
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
    pub(super) fn open_construct_at_mut(
        &mut self,
        parenthesis_depth: usize,
    ) -> Option<&mut CaseFrame> {
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

    /// Record a quoted token that left no characters in the word buffer.
    ///
    /// A quote ends the fragment being accumulated and belongs to the same
    /// token, so a wholly quoted word never fills `word`. The phase alone says
    /// what the token completed: the header's first word is the subject, and a
    /// token inside a pattern list starts that list.
    fn record_quoted_token(&mut self, parenthesis_depth: usize) {
        if let Some(frame) = self.open_construct_at_mut(parenthesis_depth) {
            match frame.phase {
                CasePhase::Header if !frame.subject_seen => frame.subject_seen = true,
                CasePhase::Pattern => frame.pattern_started = true,
                CasePhase::Header | CasePhase::Arm => {}
            }
        }
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
                if frame.is_closed_by(parenthesis_depth, word, self.spelling.is_literal()) =>
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
    pub(super) fn close_construct_at(&mut self, parenthesis_depth: usize) {
        if let Some(index) = self
            .frames
            .iter()
            .rposition(|frame| frame.depth == parenthesis_depth)
        {
            self.frames.remove(index);
        }
    }
}

/// Record what the tracker knows about the token being read.
///
/// The spelling decides whether the completed word can be a reserved word, and
/// whether a token exists at all: quoted characters never enter the word
/// buffer, so a wholly quoted token would otherwise leave no trace. One value
/// carries both facts, so the tracker's flags stay countable.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TokenSpelling {
    /// No token has begun.
    NotStarted,
    /// The token so far is unquoted word data.
    Bare,
    /// The token holds a quote or an escape of a real character.
    Quoted,
}

impl TokenSpelling {
    /// Report whether a word completed now is spelled bare.
    pub(super) const fn is_literal(self) -> bool {
        matches!(self, Self::Bare)
    }
}
