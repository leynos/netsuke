//! Cross-check the classifier against an independent byte-level state machine.
//!
//! This oracle is kept apart from the witness tests so each module owns one
//! concern: the oracle states the rule, and the tests apply it.

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
    /// Return the state and index that follow `bytes[index]`.
    ///
    /// `None` is reserved for "the scan found a bare carriage return", so the
    /// caller can stop immediately. Each state decides on its own in a helper,
    /// which keeps every function to a single flat decision; nesting the five
    /// state tables here instead buries the transition for one class inside
    /// the arms for the others.
    fn advance(self, bytes: &[u8], index: usize, byte: u8) -> Option<(Self, usize)> {
        match self {
            Self::Ground => Self::advance_ground(bytes, index, byte),
            Self::Escape => Some(Self::advance_escape(index, byte)),
            Self::ControlSequence => Some(Self::advance_control_sequence(index, byte)),
            Self::OscString => Some(Self::advance_osc(bytes, index, byte)),
            Self::ControlString => Some(Self::advance_control_string(bytes, index, byte)),
        }
    }

    /// Classify one byte in the ground state.
    ///
    /// A carriage return is a redraw unless the next byte is a line feed, in
    /// which case the pair is an ordinary line ending.
    fn advance_ground(bytes: &[u8], index: usize, byte: u8) -> Option<(Self, usize)> {
        match byte {
            b'\x1b' => Some((Self::Escape, index + 1)),
            b'\r' if bytes.get(index + 1) != Some(&b'\n') => None,
            _ => Some((Self::Ground, index + 1)),
        }
    }

    /// Classify the byte after an escape introducer.
    ///
    /// A final byte in 0x30-0x7e closes a two-byte escape; the four
    /// multi-byte introducers open their own class. Anything else is a stray
    /// control the introducer merely precedes, so only the introducer is spent
    /// and the byte is left for ground to classify: swallowing it here would
    /// lose a `\r` in `ESC \r`.
    fn advance_escape(index: usize, byte: u8) -> (Self, usize) {
        match byte {
            b'[' => (Self::ControlSequence, index + 1),
            b']' => (Self::OscString, index + 1),
            b'P' | b'X' | b'^' | b'_' => (Self::ControlString, index + 1),
            final_byte if (0x30..=0x7e).contains(&final_byte) => (Self::Ground, index + 1),
            _ => (Self::Ground, index),
        }
    }

    /// Classify one byte inside a control sequence.
    ///
    /// Parameters and intermediates are consumed; a final byte closes the
    /// sequence; anything else is a stray control, left for ground to classify.
    fn advance_control_sequence(index: usize, byte: u8) -> (Self, usize) {
        match byte {
            0x20..=0x3f => (Self::ControlSequence, index + 1),
            0x40..=0x7e => (Self::Ground, index + 1),
            _ => (Self::Ground, index),
        }
    }

    /// Classify one byte inside an OSC string.
    ///
    /// Only OSC ends on a BEL; ST ends it too, which is why the introducer
    /// pair `ESC \` is consumed as a unit.
    fn advance_osc(bytes: &[u8], index: usize, byte: u8) -> (Self, usize) {
        match byte {
            b'\x1b' if bytes.get(index + 1) == Some(&b'\\') => (Self::Ground, index + 2),
            b'\x07' => (Self::Ground, index + 1),
            _ => (Self::OscString, index + 1),
        }
    }

    /// Classify one byte inside a DCS, SOS, PM or APC string.
    ///
    /// A BEL is payload here, unlike in an OSC, so only ST or a CAN/SUB
    /// cancellation ends the string.
    fn advance_control_string(bytes: &[u8], index: usize, byte: u8) -> (Self, usize) {
        match byte {
            b'\x1b' if bytes.get(index + 1) == Some(&b'\\') => (Self::Ground, index + 2),
            0x18 | 0x1a => (Self::Ground, index + 1),
            _ => (Self::ControlString, index + 1),
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
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let Some((next_state, next_index)) = state.advance(bytes, index, byte) else {
            return true;
        };
        state = next_state;
        index = next_index;
    }
    false
}
