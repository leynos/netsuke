//! Exercise terminal-byte classification with witness tokens and generated streams.

// The oracle lives beside this file rather than under a `classify_tests/`
// directory, so the path is explicit instead of relying on the default layout.
#[path = "classify_oracle.rs"]
mod oracle;

use super::{TerminalBytes, classify};
use oracle::oracle_has_bare_carriage_return;
use proptest::prelude::*;
use rstest::rstest;

#[derive(Debug, Clone, Copy)]
enum Token {
    Ascii,
    UnicodeGlyph,
    BidiIsolates,
    Sgr,
    CursorMove,
    EraseLine,
    PrivateMode,
    SavedCursor,
    OscBel,
    OscStringTerminator,
    BareCarriageReturn,
    CarriageReturnLineFeed,
    OtherEscape,
    InvalidUtf8,
}

impl Token {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Ascii => b"Stage 1/6",
            Self::UnicodeGlyph => "✓".as_bytes(),
            Self::BidiIsolates => "\u{2068}value\u{2069}".as_bytes(),
            Self::Sgr => b"\x1b[38;5;208m",
            Self::CursorMove => b"\x1b[2A",
            Self::EraseLine => b"\x1b[K",
            Self::PrivateMode => b"\x1b[?25l",
            Self::SavedCursor => b"\x1b7\x1b8",
            Self::OscBel => b"\x1b]8;;https://example.test\x07",
            Self::OscStringTerminator => b"\x1b]8;;https://example.test\x1b\\",
            Self::BareCarriageReturn => b"progress\rnext",
            Self::CarriageReturnLineFeed => b"line\r\n",
            Self::OtherEscape => b"\x1bc",
            Self::InvalidUtf8 => &[0xff],
        }
    }

    fn expected(self) -> TerminalBytes {
        let mut expected = TerminalBytes::default();
        match self {
            Self::UnicodeGlyph => expected.has_non_ascii_glyphs = true,
            Self::BidiIsolates => expected.has_bidi_isolates = true,
            Self::Sgr => {
                expected.has_any_escape = true;
                expected.has_sgr = true;
            }
            Self::CursorMove | Self::EraseLine | Self::PrivateMode | Self::SavedCursor => {
                expected.has_any_escape = true;
                expected.has_redraw = true;
            }
            Self::OscBel | Self::OscStringTerminator => {
                expected.has_any_escape = true;
                expected.has_osc = true;
            }
            Self::BareCarriageReturn => expected.has_bare_carriage_return = true,
            Self::OtherEscape => expected.has_any_escape = true,
            Self::Ascii | Self::CarriageReturnLineFeed | Self::InvalidUtf8 => {}
        }
        expected
    }
}

#[rstest]
#[case::ascii(Token::Ascii)]
#[case::unicode_glyph(Token::UnicodeGlyph)]
#[case::bidi_isolates(Token::BidiIsolates)]
#[case::sgr(Token::Sgr)]
#[case::cursor_move(Token::CursorMove)]
#[case::erase_line(Token::EraseLine)]
#[case::private_mode(Token::PrivateMode)]
#[case::saved_cursor(Token::SavedCursor)]
#[case::osc_bel(Token::OscBel)]
#[case::osc_string_terminator(Token::OscStringTerminator)]
#[case::bare_carriage_return(Token::BareCarriageReturn)]
#[case::carriage_return_line_feed(Token::CarriageReturnLineFeed)]
#[case::other_escape(Token::OtherEscape)]
#[case::invalid_utf8(Token::InvalidUtf8)]
fn classifier_witnesses_each_token(#[case] token: Token) {
    assert_eq!(classify(token.bytes()), token.expected());
}

/// A truncated escape must not swallow the control bytes inside it.
///
/// `\x1b[\r` is a CSI introducer followed by a bare carriage return and no
/// final byte. The escaped stream carries live redraw evidence, and a
/// classifier that treats the whole unterminated sequence as opaque reports
/// that there is none — the failure is invisible in a green test, because the
/// assertion that would catch it is the one being skipped.
#[rstest]
#[case::truncated_csi_then_carriage_return(b"\x1b[\r")]
#[case::truncated_csi_then_ordinary_byte(b"\x1b[hello")]
#[case::truncated_csi_then_bell(b"\x1b[\x07")]
fn truncated_escape_does_not_hide_a_bare_carriage_return(#[case] bytes: &[u8]) {
    // Only the first case carries a carriage return; the others are controls
    // that fix the consumption point rather than the reported fact. They are
    // here so a fix that simply consumed one byte instead of the sequence
    // would still be caught by the witness above.
    let facts = classify(bytes);
    assert_eq!(
        facts.has_bare_carriage_return,
        bytes.contains(&b'\r'),
        "bare carriage return must be reported exactly when one is present: {facts:?}"
    );
    assert!(facts.has_any_escape, "the introducer is still an escape");
}

/// An OSC body is opaque payload, so a carriage return inside one is not a redraw.
///
/// This is the boundary the property test deliberately excludes from its
/// alphabet, so it is pinned here instead. Hyperlink payloads and window
/// titles legitimately contain arbitrary bytes; reporting a bare carriage
/// return for them would make the redraw assertion fire on streams that never
/// redraw.
#[rstest]
#[case::bel_terminated(b"\x1b]0;title\rwith-cr\x07")]
#[case::string_terminated(b"\x1b]8;;http://x\r/\x1b\\")]
fn osc_payload_hides_its_carriage_returns(#[case] bytes: &[u8]) {
    let facts = classify(bytes);
    assert!(facts.has_osc, "the OSC must still be detected: {facts:?}");
    assert!(
        !facts.has_bare_carriage_return,
        "a carriage return inside an OSC payload is not a redraw: {facts:?}"
    );
}

/// Every escapable string kind hides its payload, and only OSC is an OSC.
///
/// DCS, SOS, PM and APC payloads are not rendered, so a carriage return inside
/// one is string content. They are not OSC sequences, though, and reporting
/// them as such would claim a hyperlink or title where there is none.
#[rstest]
#[case::apc(b"\x1b_payload\rmore\x1b\\")]
#[case::dcs(b"\x1bPpayload\rmore\x1b\\")]
#[case::sos(b"\x1bXpayload\rmore\x1b\\")]
#[case::pm(b"\x1b^payload\rmore\x1b\\")]
fn control_string_payload_hides_its_carriage_returns(#[case] bytes: &[u8]) {
    let facts = classify(bytes);
    assert!(
        facts.has_any_escape,
        "the introducer is still an escape: {facts:?}"
    );
    assert!(
        !facts.has_bare_carriage_return,
        "a carriage return inside a control string is not a redraw: {facts:?}"
    );
    assert!(
        !facts.has_osc,
        "only an OSC introducer sets has_osc: {facts:?}"
    );
}

/// A BEL is payload inside DCS, SOS, PM and APC, unlike in an OSC.
///
/// Reusing the OSC scanner unchanged would end the string at the BEL and then
/// classify the rest of the payload as live controls, which is the defect this
/// witness pins. Only ST terminates these strings.
#[rstest]
#[case::apc(b"\x1b_payload\x07\rmore\x1b\\")]
#[case::dcs(b"\x1bPpayload\x07\rmore\x1b\\")]
fn bel_does_not_terminate_a_control_string(#[case] bytes: &[u8]) {
    let facts = classify(bytes);
    assert!(
        !facts.has_bare_carriage_return,
        "the carriage return after the BEL is still inside the payload: {facts:?}"
    );
}

/// A CAN or SUB cancels the string, so the terminal renders what follows.
#[rstest]
#[case::can(b"\x1b_payload\x18next\rmore")]
#[case::sub(b"\x1b_payload\x1anext\rmore")]
fn cancel_ends_a_control_string(#[case] bytes: &[u8]) {
    let facts = classify(bytes);
    assert!(
        facts.has_bare_carriage_return,
        "a carriage return after the cancellation is live: {facts:?}"
    );
}

proptest! {
    /// Compare bare-carriage-return detection against a byte-level oracle.
    ///
    /// The oracle in `oracle.rs` is deliberately independent of the classifier:
    /// it is a state machine over the same bytes, where the classifier is a set
    /// of scanners that each return a consumed length. The two shapes fail
    /// differently, so agreement is evidence that two implementations concur
    /// rather than one test restating itself.
    ///
    /// The alphabet covers every introducer the classifier knows: CSI, OSC and
    /// the four control-string introducers, plus both terminators, the two
    /// cancellations, a BEL and a bare carriage return. Generating only CSI
    /// bytes would leave the payload rules for OSC, DCS, SOS, PM and APC
    /// unexercised here, and a regression that scanned one of them as live
    /// controls would still pass.
    #[test]
    fn classifier_reports_bare_carriage_returns_that_the_oracle_finds(
        bytes in prop::collection::vec(
            prop::sample::select(vec![
                0x1b_u8, b'[', b'\\', b']', b'P', b'X', b'^', b'_', b'\x07', 0x18, 0x1a,
                b'\r', b'\n', b'm', b'7', b'A', b'0', b'x', b'8', b'?', b';', 0x20, 0x2f,
                0x3f,
            ]),
            0..64,
        )
    ) {
        prop_assert_eq!(
            classify(&bytes).has_bare_carriage_return,
            oracle_has_bare_carriage_return(&bytes),
            "stream {:?}",
            bytes
        );
    }

    /// Complete and truncated escapable strings both hide their payload.
    ///
    /// The payload alphabet starts at `0x20`, so it holds no control byte that
    /// could terminate the string early: the `\r` is the only control inside,
    /// and it is placed by the generator rather than drawn from the pool. An
    /// unterminated string runs to end-of-stream, which is what a capture cut
    /// short mid-payload looks like, so both cases must report no redraw.
    #[test]
    fn escapable_strings_hide_payload_carriage_returns(
        introducer in prop::sample::select(vec![b']', b'P', b'X', b'^', b'_']),
        head in prop::collection::vec(0x20_u8..0x7f, 0..8),
        tail in prop::collection::vec(0x20_u8..0x7f, 0..8),
        is_terminated in prop::bool::ANY,
    ) {
        let mut bytes = vec![0x1b, introducer];
        bytes.extend(head);
        bytes.push(b'\r');
        bytes.extend(tail);
        if is_terminated {
            bytes.extend_from_slice(b"\x1b\\");
        }

        let facts = classify(&bytes);
        prop_assert!(facts.has_any_escape, "stream {:?}", bytes);
        prop_assert!(
            !facts.has_bare_carriage_return,
            "a carriage return inside an escapable string is not a redraw: {:?}",
            bytes
        );
    }

    #[test]
    fn classifier_accepts_random_sgr_parameter_bytes(
        parameters in prop::collection::vec(0x30_u8..0x40, 0..16)
    ) {
        let mut bytes = b"\x1b[".to_vec();
        bytes.extend(parameters);
        bytes.push(b'm');

        let expected = TerminalBytes {
            has_sgr: true,
            has_any_escape: true,
            ..TerminalBytes::default()
        };
        prop_assert_eq!(classify(&bytes), expected);
    }

    #[test]
    fn classifier_matches_generated_token_streams(
        tokens in prop::collection::vec(
            prop::sample::select(vec![
                Token::Ascii,
                Token::UnicodeGlyph,
                Token::BidiIsolates,
                Token::Sgr,
                Token::CursorMove,
                Token::EraseLine,
                Token::PrivateMode,
                Token::SavedCursor,
                Token::OscBel,
                Token::OscStringTerminator,
                Token::BareCarriageReturn,
                Token::CarriageReturnLineFeed,
                Token::OtherEscape,
                Token::InvalidUtf8,
            ]),
            0..40,
        )
    ) {
        let mut bytes = Vec::new();
        let mut expected = TerminalBytes::default();
        for token in tokens {
            bytes.extend_from_slice(token.bytes());
            let token_facts = token.expected();
            expected.has_sgr |= token_facts.has_sgr;
            expected.has_redraw |= token_facts.has_redraw;
            expected.has_osc |= token_facts.has_osc;
            expected.has_any_escape |= token_facts.has_any_escape;
            expected.has_bare_carriage_return |= token_facts.has_bare_carriage_return;
            expected.has_bidi_isolates |= token_facts.has_bidi_isolates;
            expected.has_non_ascii_glyphs |= token_facts.has_non_ascii_glyphs;
        }
        prop_assert_eq!(classify(&bytes), expected);
    }
}
