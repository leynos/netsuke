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

use super::posix_lexical::SourceChar;

mod state;

use state::{CaseSightings, PrefixSlot, WordRole};

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
pub(super) struct CaseNesting {
    /// Record the parenthesis depth at which each open `case` began.
    ///
    /// A pattern terminator stands at the depth of the group that contains its
    /// `case`, which need not be the substitution's own outermost level, so the
    /// depth has to travel with the construct rather than be assumed.
    depths: Vec<usize>,
    /// Accumulate the current unquoted word so reserved words are recognized.
    word: String,
    /// Record that the traversal is inside an unquoted `#` comment.
    in_comment: bool,
    /// Record whether the next unquoted word would stand in command position.
    expects_command: bool,
    /// Record which `case` keywords have been seen and where.
    sightings: CaseSightings,
    /// Record which compound-command prefix slot the next word may fill.
    prefix_slot: PrefixSlot,
}
impl CaseNesting {
    /// Initialize an empty tracker for one command substitution.
    pub(super) const fn new() -> Self {
        Self {
            depths: Vec::new(),
            word: String::new(),
            in_comment: false,
            expects_command: true,
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
        let is_command_position = self.expects_command;
        let word = self.word.as_str();
        if word == "case" {
            return if is_command_position {
                WordRole::OpenCase
            } else {
                // A `case` behind a word the tracker read as an argument may
                // still be a keyword: `time` options and a `coproc` name are
                // opaque when written as quoted or expanded words. Recording it
                // lets the boundary check fail closed instead of lowering a
                // marker for a context the shell does not share.
                WordRole::UnplacedCase
            };
        }
        // A case pattern may itself be spelled `esac`. Only a construct open at
        // this depth is the one this word closes; anything deeper is inside a
        // pattern group and must not close it.
        let is_open_here = is_command_position && self.is_open_at(parenthesis_depth);
        if word == "esac" && is_open_here {
            return WordRole::CloseCase;
        }
        if word == "time" && is_command_position {
            return WordRole::TimePrefix;
        }
        if word == "coproc" && is_command_position {
            return WordRole::CoprocPrefix;
        }
        if self.prefix_slot == PrefixSlot::TimeOption && is_option_word(word) {
            return WordRole::TimePrefix;
        }
        if self.prefix_slot == PrefixSlot::CoprocName && !starts_command_list(word) {
            return WordRole::CoprocPrefix;
        }
        WordRole::Other {
            continues_command_list: is_command_position && starts_command_list(word),
        }
    }

    /// Apply a classified word to the tracker's state.
    fn apply_role(&mut self, role: WordRole, parenthesis_depth: usize) {
        match role {
            WordRole::OpenCase => {
                self.depths.push(parenthesis_depth);
                self.sightings.record_placed();
                self.expects_command = false;
            }
            WordRole::UnplacedCase => {
                self.sightings.record_unplaced();
                self.expects_command = false;
            }
            WordRole::CloseCase => {
                self.depths.pop();
                self.expects_command = false;
            }
            WordRole::TimePrefix => {
                self.prefix_slot = PrefixSlot::TimeOption;
                self.expects_command = true;
            }
            WordRole::CoprocPrefix => {
                self.prefix_slot = PrefixSlot::CoprocName;
                self.expects_command = true;
            }
            WordRole::Other {
                continues_command_list,
            } => self.expects_command = continues_command_list,
        }
    }
}

/// Report whether `ch` ends the unquoted word being accumulated.
///
/// A quote character ends the word as surely as a separator does: the quoted
/// text that follows is not word data, so a reserved word cannot continue into
/// it. Both categories are kept in one predicate because the caller treats them
/// identically.
const fn ends_word(ch: char) -> bool {
    SourceChar::new(ch).is_word_terminator() || matches!(ch, '\'' | '"' | '`')
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
///
/// `{` belongs here because it opens a brace-group command list in POSIX and
/// Bash. A `case` written as the first command of such a group is a reserved
/// word exactly as it is at the start of the substitution, so overlooking `{`
/// would let the pattern terminator close the substitution early and lower a
/// marker in the arm for the enclosing quote context.
fn starts_command_list(word: &str) -> bool {
    matches!(
        word,
        "if" | "then" | "else" | "elif" | "while" | "until" | "do" | "!" | "time" | "{"
    )
}

/// Report whether `word` may be an option of a `time` command.
///
/// Bash accepts `-p` and `--`; any other hyphen-led word is treated as an
/// option here too. Over-recognizing an option only keeps a command position
/// open longer, where a mistake causes a rejection rather than a mis-encoded
/// path.
fn is_option_word(word: &str) -> bool {
    word.starts_with('-') && word != "-"
}
