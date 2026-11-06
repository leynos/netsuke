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

proptest! {
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
