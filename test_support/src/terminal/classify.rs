//! Identify terminal-control sequences and non-ASCII glyphs in captured bytes.

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;

/// Facts about terminal-control bytes and visible text in one captured stream.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the classifier contract exposes independent byte-level facts"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalBytes {
    /// Whether the stream contains an ANSI Select Graphic Rendition sequence.
    pub has_sgr: bool,
    /// Whether the stream contains cursor movement, erasure, or cursor state changes.
    pub has_redraw: bool,
    /// Whether the stream contains an Operating System Command sequence.
    pub has_osc: bool,
    /// Whether the stream contains any escape byte, including incomplete sequences.
    pub has_any_escape: bool,
    /// Whether the stream contains a carriage return not followed by a line feed.
    pub has_bare_carriage_return: bool,
    /// Whether valid UTF-8 text contains a bidirectional isolate mark.
    pub has_bidi_isolates: bool,
    /// Whether valid UTF-8 text contains a non-ASCII character other than an isolate mark.
    pub has_non_ascii_glyphs: bool,
}

/// Classify terminal controls, bidi isolates, and visible non-ASCII text.
///
/// Invalid UTF-8 does not count as a glyph. Escape detection operates directly
/// on bytes, so malformed or incomplete control sequences remain observable.
///
/// # Examples
///
/// ```
/// let facts = test_support::terminal::classify(b"\x1b[38;5;208mready\x1b[0m");
/// assert!(facts.has_sgr);
/// assert!(facts.has_any_escape);
/// assert!(!facts.has_redraw);
/// ```
#[must_use]
pub fn classify(bytes: &[u8]) -> TerminalBytes {
    let mut facts = TerminalBytes::default();
    classify_controls(bytes, &mut facts);
    classify_text(bytes, &mut facts);
    facts
}

/// Scan control bytes and advance past each complete or incomplete escape.
///
/// # Examples
///
/// ```ignore
/// let facts = test_support::terminal::classify(b"\x1b]8;;https://example.test\x1b\\");
/// assert!(facts.has_osc);
/// ```
fn classify_controls(bytes: &[u8], facts: &mut TerminalBytes) {
    let mut index = 0;
    while let Some(byte) = bytes.get(index).copied() {
        match byte {
            b'\x1b' => {
                facts.has_any_escape = true;
                let remaining = bytes.get(index..).unwrap_or_default();
                index += classify_escape(remaining, facts);
            }
            b'\r' if bytes.get(index + 1) != Some(&b'\n') => {
                facts.has_bare_carriage_return = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
}

/// Classify one escape sequence and return the number of consumed bytes.
///
/// # Examples
///
/// ```ignore
/// let facts = test_support::terminal::classify(b"\x1b[2A");
/// assert!(facts.has_redraw);
/// ```
fn classify_escape(bytes: &[u8], facts: &mut TerminalBytes) -> usize {
    match bytes.get(1) {
        Some(b'[') => classify_csi(bytes.get(2..).unwrap_or_default(), facts) + 2,
        Some(b']') => {
            facts.has_osc = true;
            osc_length(bytes.get(2..).unwrap_or_default()) + 2
        }
        Some(b'7' | b'8') => {
            facts.has_redraw = true;
            2
        }
        // A two-byte escape ends in a final byte from 0x30-0x7e. Anything else
        // is a stray control that the introducer merely precedes, so only the
        // introducer is consumed and the caller classifies the next byte on its
        // own. Consuming `\x1b\r` as one escape would report a stream with a
        // bare carriage return as having none.
        Some(byte) if (0x30..=0x7e).contains(byte) => 2,
        // Both a stray control and the end of the stream consume only the
        // introducer, so the caller resumes at the next byte or stops.
        Some(_) | None => 1,
    }
}

/// Classify a Control Sequence Introducer and return its consumed length.
///
/// # Examples
///
/// ```ignore
/// let facts = test_support::terminal::classify(b"\x1b[?25l");
/// assert!(facts.has_redraw);
/// ```
fn classify_csi(bytes: &[u8], facts: &mut TerminalBytes) -> usize {
    // A CSI is parameters (0x30-0x3f) and intermediates (0x20-0x2f) up to one
    // final byte (0x40-0x7e). Scanning for the first final byte alone would
    // step over a control that cannot belong to the sequence, so a truncated
    // capture such as `\x1b[\r` would consume the carriage return and report
    // no redraw where the stream plainly contains one. Stopping at the first
    // byte outside the parameter range leaves that control for the caller,
    // which classifies it and resumes after it.
    let Some((index, byte)) = bytes
        .iter()
        .enumerate()
        .find(|(_, byte)| !(0x20..=0x3f).contains(*byte))
    else {
        return bytes.len();
    };

    if !(0x40..=0x7e).contains(byte) {
        return index;
    }

    facts.has_sgr |= *byte == b'm';
    facts.has_redraw |= is_redraw_csi_final(*byte);
    index + 1
}

/// Return whether a CSI final byte changes cursor or screen state.
///
/// # Examples
///
/// ```ignore
/// assert!(test_support::terminal::classify(b"\x1b[K").has_redraw);
/// ```
const fn is_redraw_csi_final(byte: u8) -> bool {
    matches!(
        byte,
        b'A'..=b'H'
            | b'J'
            | b'K'
            | b'S'
            | b'T'
            | b'd'
            | b'e'
            | b'f'
            | b'h'
            | b'l'
            | b'r'
            | b's'
            | b'u'
    )
}

/// Return the number of bytes in an OSC body, including its terminator.
///
/// # Examples
///
/// ```ignore
/// let facts = test_support::terminal::classify(b"\x1b]8;;url\x07");
/// assert!(facts.has_osc);
/// ```
fn osc_length(bytes: &[u8]) -> usize {
    let mut index = 0;
    while let Some(byte) = bytes.get(index).copied() {
        match byte {
            b'\x07' => return index + 1,
            b'\x1b' if bytes.get(index + 1) == Some(&b'\\') => return index + 2,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// Classify valid non-ASCII text without counting bidi control marks as glyphs.
///
/// # Examples
///
/// ```ignore
/// let facts = test_support::terminal::classify("✓".as_bytes());
/// assert!(facts.has_non_ascii_glyphs);
/// ```
fn classify_text(bytes: &[u8], facts: &mut TerminalBytes) {
    let mut remaining = bytes;
    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(text) => {
                classify_text_segment(text, facts);
                break;
            }
            Err(error) => {
                let valid_length = error.valid_up_to();
                let valid_segment = remaining
                    .get(..valid_length)
                    .and_then(|segment| std::str::from_utf8(segment).ok());
                if let Some(text_segment) = valid_segment {
                    classify_text_segment(text_segment, facts);
                }
                let invalid_length = error.error_len().unwrap_or(remaining.len() - valid_length);
                let next_index = valid_length.saturating_add(invalid_length);
                remaining = remaining.get(next_index..).unwrap_or_default();
            }
        }
    }
}

/// Record glyph facts from one valid UTF-8 segment.
///
/// # Examples
///
/// ```ignore
/// let facts = test_support::terminal::classify("✓".as_bytes());
/// assert!(facts.has_non_ascii_glyphs);
/// ```
fn classify_text_segment(text: &str, facts: &mut TerminalBytes) {
    for character in text.chars() {
        match character as u32 {
            0x2066..=0x2069 => facts.has_bidi_isolates = true,
            _ if !character.is_ascii() => facts.has_non_ascii_glyphs = true,
            _ => {}
        }
    }
}
