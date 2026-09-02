//! Bounded Kani proofs for command interpolation placeholder matching.
//!
//! The sigil proof checks every symbolic `$` position in an eight-character
//! window. `find_substitution` reads no wider context for `$in` or `$out`, so
//! that window covers the boundary contract at any command length. A separate
//! proof drives the marker fallback through the public matcher using the
//! production marker tokens.

use super::*;

const ALPHABET_T: [u8; 10] = *b"$inout_`a ";
const ALPHABET_M: [u8; 17] = *b"_NETSUKIPLACHODRX";

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
#[kani::unwind(34)]
fn marker_token_match_is_exact() {
    let chars = symbolic_chars::<32>(is_marker_byte);
    let pos = symbolic_position(chars.len());
    kani::assume(chars[pos] == '_');
    let actual = find_substitution(&chars, pos, "I", "O");
    let expected = expected_marker_match(&chars, pos);

    kani::assert(actual == expected, "marker match agrees with exact text");
    kani::cover!(
        actual == Some(("I", INS_TOKEN.len()))
            && pos > 0
            && chars.get(pos - 1).is_some_and(|ch| is_ident(*ch)),
        "marker ignores prefix boundaries"
    );
    kani::cover!(
        pos == 0 && actual == Some(("O", OUTS_TOKEN.len())),
        "marker matches at start"
    );
    kani::cover!(
        marker_near_miss(&chars, pos),
        "marker near miss is rejected"
    );
    kani::cover!(
        pos + OUTS_TOKEN.len() > chars.len() && actual.is_none(),
        "marker truncation is rejected"
    );
}

/// Build symbolic character arrays from a bounded byte alphabet.
fn symbolic_chars<const N: usize>(is_allowed: fn(u8) -> bool) -> [char; N] {
    let bytes: [u8; N] = kani::any();
    for byte in bytes {
        kani::assume(is_allowed(byte));
    }
    bytes.map(char::from)
}

/// Choose a symbolic position within a fixed character array.
fn symbolic_position(len: usize) -> usize {
    let pos = kani::any::<usize>();
    kani::assume(pos < len);
    pos
}

/// Report whether `byte` belongs to the sigil-proof alphabet.
fn is_template_byte(byte: u8) -> bool {
    ALPHABET_T.contains(&byte)
}

/// Report whether `byte` distinguishes a marker-matching behaviour class.
fn is_marker_byte(byte: u8) -> bool {
    ALPHABET_M.contains(&byte)
}

/// Specify the expected sigil replacement at one character position.
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

/// Report whether `$in` starts at `pos`.
fn has_input_pattern(chars: &[char], pos: usize) -> bool {
    chars.get(pos + 1) == Some(&'i') && chars.get(pos + 2) == Some(&'n')
}

/// Report whether `$out` starts at `pos`.
fn has_output_pattern(chars: &[char], pos: usize) -> bool {
    chars.get(pos + 1) == Some(&'o')
        && chars.get(pos + 2) == Some(&'u')
        && chars.get(pos + 3) == Some(&'t')
}

/// Report whether the preceding character permits a sigil placeholder.
fn leading_boundary(chars: &[char], pos: usize) -> bool {
    chars
        .get(pos.wrapping_sub(1))
        .is_none_or(|ch| !is_ident(*ch))
}

/// Report whether the character after a sigil placeholder is valid.
fn trailing_boundary(chars: &[char], pos: usize, token_len: usize) -> bool {
    chars
        .get(pos + token_len + 1)
        .is_none_or(|ch| !is_ident(*ch))
}

/// Report whether `ch` is an ASCII identifier character.
const fn is_ident(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Specify the marker fallback result at one character position.
fn expected_marker_match(chars: &[char], pos: usize) -> Option<(&'static str, usize)> {
    marker_matches(chars, pos, INS_TOKEN)
        .then_some(("I", INS_TOKEN.len()))
        .or_else(|| marker_matches(chars, pos, OUTS_TOKEN).then_some(("O", OUTS_TOKEN.len())))
}

/// Report whether `token` matches exactly at `pos`.
fn marker_matches(chars: &[char], pos: usize, token: &str) -> bool {
    token
        .chars()
        .enumerate()
        .all(|(offset, character)| chars.get(pos + offset) == Some(&character))
}

/// Report whether an underscore starts a non-matching marker candidate.
fn marker_near_miss(chars: &[char], pos: usize) -> bool {
    chars.get(pos) == Some(&'_') && expected_marker_match(chars, pos).is_none()
}
