//! Hold the state values that describe one substitution's `case` tracking.
//!
//! Each value answers one question: which `case` keywords the tracker saw,
//! which compound-command prefix is open, what a completed word does to the
//! state, and which part of an open construct the tracker is reading. The
//! construct also owns its own operator transitions, because only it knows
//! whether a `|`, `)`, `;;`, or `;&` written at its depth is pattern syntax or
//! an ordinary shell operator. Keeping all of that here leaves the tracker
//! module to the word and comment scanning that drives them.

/// Record which `case` keywords one substitution's tracker has seen.
///
/// The two observations are independent, so they cannot be one flag: a single
/// substitution may contain a modelled construct, an unplaceable keyword, or
/// both. They are one value because every reader asks about the same
/// substitution's sightings, and the boundary check needs `has_any` and
/// `has_unplaced` together.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CaseSightings {
    /// No `case` keyword has been seen.
    None,
    /// Every `case` keyword seen stood in a modelled command position.
    Placed,
    /// A `case` keyword stood where the tracker read argument data.
    Unplaced,
    /// A placed keyword and an unplaced keyword have both been seen.
    PlacedAndUnplaced,
}

impl CaseSightings {
    /// Record that a `case` keyword stood in command position.
    pub(super) const fn record_placed(&mut self) {
        *self = match *self {
            Self::None | Self::Placed => Self::Placed,
            Self::Unplaced | Self::PlacedAndUnplaced => Self::PlacedAndUnplaced,
        };
    }

    /// Record that a `case` keyword stood where the tracker read argument data.
    pub(super) const fn record_unplaced(&mut self) {
        *self = match *self {
            Self::None | Self::Unplaced => Self::Unplaced,
            Self::Placed | Self::PlacedAndUnplaced => Self::PlacedAndUnplaced,
        };
    }

    /// Report whether a `case` keyword was seen in either position.
    pub(super) const fn has_any(self) -> bool {
        !matches!(self, Self::None)
    }

    /// Report whether a `case` keyword stood outside a modelled position.
    pub(super) const fn has_unplaced(self) -> bool {
        matches!(self, Self::Unplaced | Self::PlacedAndUnplaced)
    }
}

/// Record which compound-command prefix slot the next word may fill.
///
/// Bash keeps a command position open after a `time` option and after a
/// `coproc` name, so the word after either may be the `case` keyword. The
/// prefixes are alternatives rather than independent flags: a word cannot be
/// both an option of `time` and the name of a `coproc`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PrefixSlot {
    /// No compound-command prefix is open.
    None,
    /// The next word may be an option of a `time` command.
    TimeOption,
    /// The next word may name a `coproc` command.
    CoprocName,
}

/// Record which part of a `case` construct the tracker is reading.
///
/// The phase decides whether a reserved word is a keyword or pattern text. Bash
/// reads `esac` as the construct's closer where a pattern would start, but as
/// part of the pattern after `|`, so the tracker has to know where it stands
/// rather than only how deeply it is nested.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CasePhase {
    /// Between `case` and `in`: the subject word, then the `in` keyword.
    Header,
    /// Inside a pattern list, where reserved words are pattern text.
    Pattern,
    /// Inside an arm's command list, where reserved words are keywords again.
    Arm,
}

/// Record one open `case` construct.
///
/// The depth travels with the construct because a pattern terminator stands at
/// the depth of the group that contains its `case`, and the phase travels with
/// it because the same word is a keyword in one part of the construct and data
/// in another.
#[derive(Clone, Copy)]
pub(super) struct CaseFrame {
    /// The parenthesis depth at which the construct opened.
    pub(super) depth: usize,
    /// The part of the construct the tracker is reading.
    pub(super) phase: CasePhase,
    /// Whether the last thing read in a pattern list was `|`.
    pub(super) after_alternative: bool,
}

impl CaseFrame {
    /// Record that the `in` keyword completed the construct's header.
    ///
    /// The pattern list that follows starts without an alternative, because a
    /// `|` seen before `in` belonged to an earlier construct.
    pub(super) const fn enter_pattern(&mut self) {
        self.phase = CasePhase::Pattern;
        self.after_alternative = false;
    }

    /// Report whether `word` terminates this construct's pattern list.
    ///
    /// Bash reads `esac` as the construct's closer only where a pattern would
    /// start, so a pattern list that just read a `|` is not closed by it.
    pub(super) fn is_closed_by(&self, parenthesis_depth: usize, word: &str) -> bool {
        let is_terminator = self.depth == parenthesis_depth && word == "esac";
        is_terminator && !self.after_alternative
    }

    /// Move the construct across an operator written at its own depth.
    ///
    /// A `|` inside a pattern list introduces another alternative, after which
    /// a reserved word such as `esac` is pattern text. The `)` that ends the
    /// pattern list starts the arm's command list, where reserved words are
    /// keywords again. The arm ends at `;;`, `;&`, or `;;&`; the tracker reads
    /// one character at a time, so the second character of the operator and the
    /// one before it are what confirm the pair.
    pub(super) const fn apply_operator(&mut self, ch: char, previous: Option<char>) {
        match ch {
            '|' if matches!(self.phase, CasePhase::Pattern) => self.after_alternative = true,
            ')' if matches!(self.phase, CasePhase::Pattern) => {
                self.phase = CasePhase::Arm;
                self.after_alternative = false;
            }
            ';' | '&' if matches!(previous, Some(';')) && matches!(self.phase, CasePhase::Arm) => {
                self.phase = CasePhase::Pattern;
                self.after_alternative = false;
            }
            _ => {}
        }
    }
}

/// Record what a completed word does to the `case` nesting state.
///
/// The roles are the transition labels of the tracker's small state machine, so
/// classifying and applying stay separate: each new command position adds one
/// variant here, one arm in
/// [`CaseNesting::apply_role`](super::CaseNesting::apply_role), and one
/// regression.
#[derive(Clone, Copy)]
pub(super) enum WordRole {
    /// A `case` keyword in command position: open a construct.
    OpenCase,
    /// The `in` keyword: the construct's header is complete and a pattern
    /// list follows.
    EnterPattern,
    /// A `case` word outside a modelled command position.
    UnplacedCase,
    /// An `esac` keyword closing the construct open at this depth.
    CloseCase,
    /// A `time` keyword or one of its options.
    TimePrefix,
    /// A `coproc` keyword or the name that follows it.
    CoprocPrefix,
    /// Any other word, carrying whether it leaves a command position open.
    Other {
        /// Whether the next word is again a command word.
        continues_command_list: bool,
    },
}
