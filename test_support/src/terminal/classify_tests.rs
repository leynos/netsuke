//! Exercise terminal-byte classification with witness tokens and generated streams.

use super::{TerminalBytes, classify};
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

proptest! {
    /// Compare bare-carriage-return detection against a byte-level oracle.
    ///
    /// The oracle is deliberately independent of the parser: it walks the
    /// stream remembering only whether a `\r` is followed by `\n`. Escape
    /// handling is not modelled at all, so agreement means the parser is not
    /// letting escape scanning suppress a control plainly present in the
    /// bytes.
    ///
    /// OSC introducers are excluded from the alphabet rather than from the
    /// oracle. An OSC body is opaque payload by definition — a `\r` inside one
    /// is not a redraw — so generating them would test a semantics question
    /// this property does not ask. Coverage of that boundary is pinned
    /// separately by `osc_payload_hides_its_carriage_returns`.
    #[test]
    fn classifier_reports_bare_carriage_returns_that_the_oracle_finds(
        bytes in prop::collection::vec(
            prop::sample::select(vec![
                0x1b_u8, b'[', b'\\', b'\r', b'\n', b'm', b'7', b'A', b'0', b'x', b'8',
                b'?', b';', b'\x07', 0x20, 0x2f, 0x3f,
            ]),
            0..64,
        )
    ) {
        let expected = bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'));
        prop_assert_eq!(
            classify(&bytes).has_bare_carriage_return,
            expected,
            "stream {:?}",
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
