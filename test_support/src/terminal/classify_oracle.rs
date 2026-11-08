//! Cross-check the classifier against an independent byte-level state machine.
//!
//! This oracle is kept apart from the witness tests so each module owns one
//! concern: the oracle states the rule, and the tests apply it.

/// A position in the stream paired with the byte found there.
///
/// The position and its byte are always needed together, so carrying them as
/// one value keeps a state handler from taking four loose parameters and makes
/// the "consume this byte" and "leave it for ground" answers read as what they
/// are: advancing the cursor, or not.
#[derive(Clone, Copy)]
struct Cursor<'a> {
    /// The stream being scanned.
    bytes: &'a [u8],
    /// The offset of `byte` within `bytes`.
    index: usize,
    /// The byte at `index`.
    byte: u8,
}

impl<'a> Cursor<'a> {
    /// Read the cursor at `index`, or `None` at end of stream.
    fn at(bytes: &'a [u8], index: usize) -> Option<Self> {
        let byte = bytes.get(index).copied()?;
        Some(Self { bytes, index, byte })
    }

    /// Return the cursor one byte further on, or `None` at end of stream.
    fn next(self) -> Option<Self> {
        Self::at(self.bytes, self.index + 1)
    }

    /// Report whether the byte after this one is `expected`.
    fn followed_by(self, expected: u8) -> bool {
        self.bytes.get(self.index + 1) == Some(&expected)
    }

    /// Consume `ESC \` as a unit, or report that it is not there.
    ///
    /// Both introducer and final byte belong to the terminator, so a caller
    /// that matched only the `ESC` would leave a stray backslash to be
    /// classified as ground text.
    fn after_string_terminator(self) -> Option<Self> {
        if self.followed_by(b'\\') {
            Self::at(self.bytes, self.index + 2)
        } else {
            None
        }
    }
}

/// Where the oracle advanced to, or that a bare carriage return ended the scan.
enum Step<'a> {
    /// Continue in `state` from `cursor`.
    Advance {
        /// The class the scan is now in.
        state: OracleState,
        /// The next position to read.
        cursor: Option<Cursor<'a>>,
    },
    /// The scan found a carriage return that is not part of a line ending.
    FoundRedraw,
}

/// The states the byte-level oracle walks, one byte at a time.
///
/// This mirrors the classes a terminal itself distinguishes, not the shape of
/// the classifier under test: the classifier is a set of scanners that each
/// return a consumed length, while this is a state machine that transitions
/// between states. Two different shapes that agree are evidence of concurrence
/// rather than one test restating itself.
#[derive(Clone, Copy)]
enum OracleState {
    Ground,
    Escape,
    ControlSequence,
    OscString,
    ControlString,
}

impl OracleState {
    /// Classify `cursor` from this state.
    ///
    /// Each state decides on its own in a helper, which keeps every function to
    /// a single flat decision; nesting the five state tables here instead
    /// buries the transition for one class inside the arms for the others.
    fn step(self, cursor: Cursor<'_>) -> Step<'_> {
        match self {
            Self::Ground => Self::ground(cursor),
            Self::Escape => Self::advance(Self::escape(cursor)),
            Self::ControlSequence => Self::advance(Self::control_sequence(cursor)),
            Self::OscString => Self::advance(Self::osc(cursor)),
            Self::ControlString => Self::advance(Self::control_string(cursor)),
        }
    }

    /// Wrap a `(state, cursor)` answer as a continuing step.
    fn advance((state, cursor): (Self, Option<Cursor<'_>>)) -> Step<'_> {
        Step::Advance { state, cursor }
    }

    /// Classify one byte in the ground state.
    ///
    /// A carriage return is a redraw unless the next byte is a line feed, in
    /// which case the pair is an ordinary line ending.
    fn ground(cursor: Cursor<'_>) -> Step<'_> {
        match cursor.byte {
            b'\x1b' => Self::advance((Self::Escape, cursor.next())),
            b'\r' if !cursor.followed_by(b'\n') => Step::FoundRedraw,
            _ => Self::advance((Self::Ground, cursor.next())),
        }
    }

    /// Classify the byte after an escape introducer.
    ///
    /// A final byte in 0x30-0x7e closes a two-byte escape; the four
    /// multi-byte introducers open their own class. Anything else is a stray
    /// control the introducer merely precedes, so only the introducer is spent
    /// and the byte is left for ground to classify: swallowing it here would
    /// lose a `\r` in `ESC \r`.
    fn escape(cursor: Cursor<'_>) -> (Self, Option<Cursor<'_>>) {
        match cursor.byte {
            b'[' => (Self::ControlSequence, cursor.next()),
            b']' => (Self::OscString, cursor.next()),
            b'P' | b'X' | b'^' | b'_' => (Self::ControlString, cursor.next()),
            final_byte if (0x30..=0x7e).contains(&final_byte) => (Self::Ground, cursor.next()),
            _ => (Self::Ground, Some(cursor)),
        }
    }

    /// Classify one byte inside a control sequence.
    ///
    /// Parameters and intermediates are consumed; a final byte closes the
    /// sequence; anything else is a stray control, left for ground to classify.
    fn control_sequence(cursor: Cursor<'_>) -> (Self, Option<Cursor<'_>>) {
        match cursor.byte {
            0x20..=0x3f => (Self::ControlSequence, cursor.next()),
            0x40..=0x7e => (Self::Ground, cursor.next()),
            _ => (Self::Ground, Some(cursor)),
        }
    }

    /// Classify one byte inside an OSC string.
    ///
    /// Only OSC ends on a BEL; ST ends it too, which is why the introducer
    /// pair `ESC \` is consumed as a unit.
    fn osc(cursor: Cursor<'_>) -> (Self, Option<Cursor<'_>>) {
        match cursor.byte {
            b'\x1b' if cursor.followed_by(b'\\') => {
                (Self::Ground, cursor.after_string_terminator())
            }
            b'\x07' => (Self::Ground, cursor.next()),
            _ => (Self::OscString, cursor.next()),
        }
    }

    /// Classify one byte inside a DCS, SOS, PM or APC string.
    ///
    /// A BEL is payload here, unlike in an OSC, so only ST or a CAN/SUB
    /// cancellation ends the string.
    fn control_string(cursor: Cursor<'_>) -> (Self, Option<Cursor<'_>>) {
        match cursor.byte {
            b'\x1b' if cursor.followed_by(b'\\') => {
                (Self::Ground, cursor.after_string_terminator())
            }
            0x18 | 0x1a => (Self::Ground, cursor.next()),
            _ => (Self::ControlString, cursor.next()),
        }
    }
}

/// Decide whether an independent byte-level scan finds a bare carriage return.
///
/// It models every escape class the classifier claims to handle, including the
/// four control-string introducers, because a `\r` inside such a payload is
/// string content and a `\r` after a cancellation is live. An escapable string
/// that never terminates is opaque to end-of-stream, which is the documented
/// rule for a truncated capture.
pub(super) fn oracle_has_bare_carriage_return(bytes: &[u8]) -> bool {
    let mut state = OracleState::Ground;
    let mut cursor = Cursor::at(bytes, 0);
    while let Some(current) = cursor {
        match state.step(current) {
            Step::Advance {
                state: next_state,
                cursor: next_cursor,
            } => {
                state = next_state;
                cursor = next_cursor;
            }
            Step::FoundRedraw => return true,
        }
    }
    false
}
