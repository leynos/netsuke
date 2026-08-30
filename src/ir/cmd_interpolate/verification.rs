//! Bounded Kani proofs for command interpolation placeholder matching.
//!
//! The sigil proof checks every symbolic `$` position in an eight-character
//! window. `find_substitution` reads no wider context for `$in` or `$out`, so
//! that window covers the boundary contract at any command length. A separate
//! proof drives the marker fallback, avoiding its unrelated long-token loop.

use super::*;

const ALPHABET_T: [u8; 10] = *b"$inout_`a ";
const ALPHABET_M: [u8; 4] = *b"_XYa";
const MARKER_TOKEN: &str = "_X_";

/// Prove sigil placeholders obey their complete boundary contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(32)]
fn sigil_placeholder_match_is_exact() {
    let chars = symbolic_chars::<8>(is_template_byte);
    let pos = symbolic_position(chars.len());
    kani::assume(chars[pos] == '$');
    let actual = find_substitution(&chars, pos, "I", "O");
    let expected = expected_sigil_match(&chars, pos, "I", "O");

    kani::assert(
        actual == expected,
        "sigil match agrees with the boundary contract",
    );
    kani::cover!(actual == Some(("I", 3)), "input placeholder matches");
    kani::cover!(actual == Some(("O", 4)), "output placeholder matches");
    kani::cover!(
        has_input_pattern(&chars, pos) && !leading_boundary(&chars, pos),
        "input rejects a leading identifier"
    );
    kani::cover!(
        has_input_pattern(&chars, pos) && !trailing_boundary(&chars, pos, 2),
        "input rejects a trailing identifier"
    );
    kani::cover!(pos == 0 && actual.is_some(), "start boundary matches");
}

/// Prove marker tokens match exactly and deliberately ignore word boundaries.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(32)]
fn marker_token_match_is_exact() {
    let chars = symbolic_chars::<6>(is_marker_byte);
    let pos = symbolic_position(chars.len());
    let actual = try_match_token(&chars, pos, MARKER_TOKEN, "R");
    let expected = expected_marker_match(&chars, pos);

    kani::assert(actual == expected, "marker match agrees with exact text");
    kani::cover!(
        actual == Some(("R", 3)) && pos > 0 && chars.get(pos - 1).is_some_and(|ch| is_ident(*ch)),
        "marker ignores prefix boundaries"
    );
    kani::cover!(
        pos == 0 && actual == Some(("R", 3)),
        "marker matches at start"
    );
    kani::cover!(
        marker_near_miss(&chars, pos),
        "marker near miss is rejected"
    );
    kani::cover!(
        pos + MARKER_TOKEN.len() > chars.len(),
        "marker truncation is rejected"
    );
}

fn symbolic_chars<const N: usize>(is_allowed: fn(u8) -> bool) -> [char; N] {
    let bytes: [u8; N] = kani::any();
    for byte in bytes {
        kani::assume(is_allowed(byte));
    }
    bytes.map(char::from)
}

fn symbolic_position(len: usize) -> usize {
    let pos = kani::any::<usize>();
    kani::assume(pos < len);
    pos
}

fn is_template_byte(byte: u8) -> bool {
    ALPHABET_T.contains(&byte)
}

fn is_marker_byte(byte: u8) -> bool {
    ALPHABET_M.contains(&byte)
}

fn expected_sigil_match<'a>(
    chars: &[char],
    pos: usize,
    ins: &'a str,
    outs: &'a str,
) -> Option<(&'a str, usize)> {
    if chars.get(pos) != Some(&'$') || !leading_boundary(chars, pos) {
        return None;
    }
    if has_input_pattern(chars, pos) && trailing_boundary(chars, pos, 2) {
        return Some((ins, 3));
    }
    if has_output_pattern(chars, pos) && trailing_boundary(chars, pos, 3) {
        return Some((outs, 4));
    }
    None
}

fn has_input_pattern(chars: &[char], pos: usize) -> bool {
    chars.get(pos + 1) == Some(&'i') && chars.get(pos + 2) == Some(&'n')
}

fn has_output_pattern(chars: &[char], pos: usize) -> bool {
    chars.get(pos + 1) == Some(&'o')
        && chars.get(pos + 2) == Some(&'u')
        && chars.get(pos + 3) == Some(&'t')
}

fn leading_boundary(chars: &[char], pos: usize) -> bool {
    chars
        .get(pos.wrapping_sub(1))
        .is_none_or(|ch| !is_ident(*ch))
}

fn trailing_boundary(chars: &[char], pos: usize, token_len: usize) -> bool {
    chars
        .get(pos + token_len + 1)
        .is_none_or(|ch| !is_ident(*ch))
}

const fn is_ident(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn expected_marker_match(chars: &[char], pos: usize) -> Option<(&'static str, usize)> {
    if marker_matches(chars, pos) {
        Some(("R", 3))
    } else {
        None
    }
}

fn marker_matches(chars: &[char], pos: usize) -> bool {
    chars.get(pos) == Some(&'_')
        && chars.get(pos + 1) == Some(&'X')
        && chars.get(pos + 2) == Some(&'_')
}

fn marker_near_miss(chars: &[char], pos: usize) -> bool {
    chars.get(pos) == Some(&'_')
        && chars.get(pos + 1) == Some(&'Y')
        && chars.get(pos + 2) == Some(&'_')
}
