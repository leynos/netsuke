//! Canonicalizer parity: the adapter's bytes against the canonicalizer itself.
//!
//! Every case here compares [`key_bytes`] against
//! `serde_json_canonicalizer::to_vec` called directly on an equivalent Rust
//! value, so the adapter is never its own oracle. This is a different kind of
//! judgement from the refusals in the sibling `boundaries` battery: those ask
//! whether an out-of-domain value is turned away, these ask whether an
//! admitted one is rendered exactly as RFC 8785 requires.

use anyhow::{Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::*;

/// Canonical JSON of the canonicalizer's own `testdata/output/structures.json`.
///
/// Copied verbatim from the crate's fixture rather than retyped from the RFC
/// text, because the escapes are the point: the member name is a real newline.
const STRUCTURES_CANONICAL: &str = concat!(
    r#"{"":"empty","1":{"\n":56,"f":{"F":5,"f":"hi"}},"10":{},"#,
    r#""111":[{"E":"no","e":"yes"}],"A":{},"a":{}}"#,
);

/// Verify every integer width agrees with the canonicalizer itself at its
/// extremes, so no width is narrowed before the canonicalizer sees it.
#[rstest]
fn extreme_integers_match_the_canonicalizer() -> Result<()> {
    for wide in signed_extremes() {
        ensure!(
            key_bytes(&Value::from(wide))? == serde_json_canonicalizer::to_vec(&wide)?,
            "signed width {wide} must agree with the canonicalizer",
        );
    }
    for wide in unsigned_extremes() {
        ensure!(
            key_bytes(&Value::from(wide))? == serde_json_canonicalizer::to_vec(&wide)?,
            "unsigned width {wide} must agree with the canonicalizer",
        );
    }
    Ok(())
}

/// Verify finite binary64 values, including the exponent boundaries, agree
/// with the canonicalizer itself.
#[rstest]
fn finite_floats_match_the_canonicalizer() -> Result<()> {
    for exact in [
        0.0,
        -0.0,
        4.5,
        2e-3,
        1e-27,
        1e30,
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
        f64::EPSILON,
    ] {
        ensure!(
            key_bytes(&Value::from(exact))? == serde_json_canonicalizer::to_vec(&exact)?,
            "binary64 {exact:e} must agree with the canonicalizer",
        );
    }
    Ok(())
}

/// Verify the binary64 collapse RFC 8785 mandates, so the behaviour is pinned
/// as specified rather than worked around.
#[rstest]
fn numbers_collapse_as_rfc_8785_specifies() -> Result<()> {
    let just_over = 9_007_199_254_740_993_i64;
    let exactly = 9_007_199_254_740_992_i64;
    ensure!(
        key_bytes(&Value::from(just_over))? == key_bytes(&Value::from(exactly))?,
        "2^53 + 1 and 2^53 must share a key",
    );
    ensure!(
        key_bytes(&Value::from(-0.0))? == key_bytes(&Value::from(0.0))?,
        "-0.0 and 0.0 must share a key",
    );
    ensure!(
        key_bytes(&Value::from(0_i64))? == key_bytes(&Value::from(0.0_f64))?,
        "an integer zero and a float zero must share a key",
    );
    ensure!(
        key_bytes(&Value::from(i128::MAX))? != key_bytes(&Value::from(u128::MAX))?,
        "the two extreme unsigned and signed values must stay distinct",
    );
    Ok(())
}

/// Verify the emitted bytes are the canonicalizer's own output on the JCS
/// sorting fixture, and that property names follow the UTF-16 ordering rule.
#[rstest]
fn emitted_bytes_match_the_canonicalizer_on_jcs_fixtures() -> Result<()> {
    let fixture = serde_json::json!({
        "1": {"f": {"f": "hi", "F": 5}, "\n": 56.0},
        "10": {},
        "": "empty",
        "a": {},
        "111": [{"e": "yes", "E": "no"}],
        "A": {},
    });
    let value = Value::from_serialize(&fixture);
    let emitted = key_bytes(&value)?;

    ensure!(
        emitted == serde_json_canonicalizer::to_vec(&fixture)?,
        "the adapter must agree with the canonicalizer on the sorting fixture",
    );
    ensure!(
        emitted == STRUCTURES_CANONICAL.as_bytes(),
        "the sorting fixture must canonicalize to {STRUCTURES_CANONICAL}; got {}",
        String::from_utf8_lossy(&emitted),
    );

    // U+1F600 encodes as the surrogate pair D83D DE00, which sorts before
    // U+FB33 by UTF-16 code unit but after it by scalar value, so only the
    // UTF-16 rule puts the emoji first.
    let astral = Value::from_serialize(serde_json::json!({ "\u{1f600}": 1, "\u{fb33}": 2 }));
    ensure!(
        key_bytes(&astral)? == "{\"\u{1f600}\":1,\"\u{fb33}\":2}".as_bytes(),
        "property names must be ordered by UTF-16 code unit",
    );
    Ok(())
}

/// Verify deduplication returns the caller's own first values, once each, in
/// first-appearance order, for classes whose order is not lexical.
///
/// The retained representatives are `zeta` then `alpha`, which is neither
/// ascending nor descending, so an output order borrowed from the membership
/// structure could not produce it.
#[rstest]
fn deduplication_returns_first_representatives_in_input_order() -> Result<()> {
    let first = ordered_map(&[("alpha", 1), ("beta", 2)]);
    let twin = ordered_map(&[("beta", 2), ("alpha", 1)]);
    let input = [
        first.clone(),
        Value::from("zeta"),
        twin,
        Value::from("alpha"),
        Value::from("zeta"),
        first,
    ];
    let retained = stable_deduplicate(&input).context("every input is admitted")?;

    ensure!(
        retained.len() == 3,
        "three distinct keys were supplied, got {}",
        retained.len(),
    );
    // Index 0 is the map, 1 is `zeta`, and 3 is `alpha`: index 2 repeats the
    // map's key and index 4 repeats `zeta`, so neither may be retained.
    let expected = [&input[0], &input[1], &input[3]];
    for (retained_value, original) in retained.iter().zip(expected) {
        ensure!(
            std::ptr::eq(*retained_value, original),
            "a retained value must be the first original, not a reconstruction",
        );
    }

    let order: Vec<String> = retained
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    ensure!(
        order == ["zeta", "alpha"],
        "output order must be first-appearance order, got {order:?}",
    );
    Ok(())
}
