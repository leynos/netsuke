//! Hold the state values that describe one substitution's `case` tracking.
//!
//! Each value answers one question: which `case` keywords the tracker saw,
//! which compound-command prefix is open, and what a completed word does to
//! the state. Keeping them here leaves the tracker module to the word and
//! comment scanning that drives them.

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

/// Record what a completed word does to the `case` nesting state.
///
/// The roles are the transition labels of the tracker's small state machine, so
/// classifying and applying stay separate: each new command position adds one
/// variant here, one arm in [`CaseNesting::apply_role`], and one regression.
#[derive(Clone, Copy)]
pub(super) enum WordRole {
    /// A `case` keyword in command position: open a construct.
    OpenCase,
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
