//! Bounded Kani proofs for command interpolation placeholder matching.
//!
//! The shell-variable proof checks every symbolic `$` position in an
//! eight-character window. Literal `$in` and `$out` must remain shell text, so
//! the recogniser must return no Netsuke substitution there. A separate proof
//! drives exact matching through the public recogniser using the production
//! marker tokens.

use super::*;

/// Prove literal shell-variable prefixes never select a Netsuke marker.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(32)]
fn shell_variable_prefix_does_not_match() {
    let chars = symbolic_chars::<8>();
    let pos = symbolic_position(chars.len());
    kani::assume(chars[pos] == '$');
    let actual = find_substitution(&chars, pos);

    kani::assert(
        actual.is_none(),
        "literal shell-variable prefixes are not Netsuke markers",
    );
    kani::cover!(
        has_input_shell_variable(&chars, pos),
        "input variable passes through"
    );
    kani::cover!(
        has_output_shell_variable(&chars, pos),
        "output variable passes through"
    );
    kani::cover!(pos == 0, "start position passes through");
}

/// Prove marker tokens match exactly and deliberately ignore word boundaries.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(34)]
fn marker_token_match_is_exact() {
    let chars = symbolic_chars::<32>();
    let pos = symbolic_position(chars.len());
    kani::assume(chars[pos] == '_');
    let actual = find_substitution(&chars, pos);
    let expected = expected_marker_match(&chars, pos);

    kani::assert(actual == expected, "marker match agrees with exact text");
    kani::cover!(
        is_input_marker_match(actual)
            && pos > 0
            && chars.get(pos - 1).is_some_and(|ch| is_ident(*ch)),
        "marker ignores prefix boundaries"
    );
    kani::cover!(
        pos == 0 && is_output_marker_match(actual),
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

/// Build symbolic character arrays from the complete byte range.
fn symbolic_chars<const N: usize>() -> [char; N] {
    let bytes: [u8; N] = kani::any();
    bytes.map(char::from)
}

/// Choose a symbolic position within a fixed character array.
fn symbolic_position(len: usize) -> usize {
    let pos = kani::any::<usize>();
    kani::assume(pos < len);
    pos
}

/// Report whether `$in` starts at `pos`.
fn has_input_shell_variable(chars: &[char], pos: usize) -> bool {
    chars.get(pos + 1) == Some(&'i') && chars.get(pos + 2) == Some(&'n')
}

/// Report whether `$out` starts at `pos`.
fn has_output_shell_variable(chars: &[char], pos: usize) -> bool {
    chars.get(pos + 1) == Some(&'o')
        && chars.get(pos + 2) == Some(&'u')
        && chars.get(pos + 3) == Some(&'t')
}

/// Report whether `ch` is an ASCII identifier character.
const fn is_ident(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Specify the marker result expected at one character position.
fn expected_marker_match(chars: &[char], pos: usize) -> Option<(Placeholder, usize)> {
    marker_matches(chars, pos, INS_TOKEN)
        .then_some((Placeholder::Inputs, INS_TOKEN.chars().count()))
        .or_else(|| {
            marker_matches(chars, pos, OUTS_TOKEN)
                .then_some((Placeholder::Outputs, OUTS_TOKEN.chars().count()))
        })
}

/// Report whether the recogniser selected the complete input marker.
fn is_input_marker_match(actual: Option<(Placeholder, usize)>) -> bool {
    matches!(actual, Some((Placeholder::Inputs, length)) if length == INS_TOKEN.chars().count())
}

/// Report whether the recogniser selected the complete output marker.
fn is_output_marker_match(actual: Option<(Placeholder, usize)>) -> bool {
    matches!(actual, Some((Placeholder::Outputs, length)) if length == OUTS_TOKEN.chars().count())
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
