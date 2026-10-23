//! Handwritten cases over the public canonical-value surface.
//!
//! Every input here is written out rather than generated, so each case
//! states one property in a form a reader can check by inspection. The
//! order-discriminating control lives here because its whole value is that
//! the two fixtures it compares are visible side by side.

use anyhow::{Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::*;

/// Control: the two reordered fixtures really do enumerate in different
/// orders, so the key equality asserted elsewhere is not an accident of the
/// fixtures, of a shared host order, or of an implementation that discards
/// order everywhere.
#[rstest]
fn reordered_map_fixtures_enumerate_in_opposite_orders() {
    let first = ordered_map(&[("alpha", 1), ("beta", 2), ("gamma", 3)]);
    let second = ordered_map(&[("gamma", 3), ("beta", 2), ("alpha", 1)]);

    assert_ne!(
        enumeration_order(&first),
        enumeration_order(&second),
        "the fixtures must enumerate differently or they discriminate nothing"
    );
}

/// Verify two string-keyed maps with identical members in different insertion
/// orders produce byte-identical keys, and a different member set does not.
#[rstest]
fn reordered_maps_share_a_key_but_different_members_do_not() -> Result<()> {
    let first = ordered_map(&[("alpha", 1), ("beta", 2), ("gamma", 3)]);
    let second = ordered_map(&[("gamma", 3), ("beta", 2), ("alpha", 1)]);
    let third = ordered_map(&[("beta", 2), ("alpha", 1), ("gamma", 3)]);

    ensure!(
        key_bytes(&first)? == key_bytes(&second)?,
        "member insertion order must not reach the canonical key",
    );
    ensure!(
        key_bytes(&first)? == key_bytes(&third)?,
        "every permutation of one member set must share a key",
    );
    ensure!(
        canonical_eq(&first, &second).context("both maps are admitted")?,
        "reordered maps must compare equal",
    );
    for unlike in [
        ordered_map(&[("alpha", 1), ("beta", 2)]),
        ordered_map(&[("alpha", 1), ("beta", 2), ("gamma", 4)]),
    ] {
        ensure!(
            key_bytes(&first)? != key_bytes(&unlike)?,
            "a different member set must have a different key",
        );
    }
    Ok(())
}

/// Control: equality is still reflexive for every admitted class, which is the
/// fault an inverted comparison would break.
///
/// Reversing the operand order is *not* a sufficient control: equality is
/// symmetric, so a reversed comparison still answers `true` for a value
/// against itself. An inversion is the fault this case catches, and it is the
/// only case here that would.
#[rstest]
fn every_admitted_class_is_reflexive() -> Result<()> {
    let admitted = [
        Value::from(()),
        Value::from(true),
        Value::from(0),
        Value::from(1_234_567_i64),
        Value::from(2.5),
        Value::from("text"),
        Value::from([1, 2, 3]),
        ordered_map(&[("a", 1)]),
    ];
    for value in &admitted {
        ensure!(
            canonical_eq(value, value).context("each class is admitted")?,
            "class {:?} must be reflexive",
            value.kind(),
        );
    }
    Ok(())
}

/// Verify sequence order is significant in both directions.
#[rstest]
fn sequence_order_is_significant() -> Result<()> {
    let ascending = Value::from([1, 2, 3]);
    let descending = Value::from([3, 2, 1]);

    ensure!(
        !canonical_eq(&ascending, &descending).context("both sequences are admitted")?,
        "sequence order must be significant",
    );
    ensure!(
        canonical_eq(&ascending, &Value::from([1, 2, 3])).context("both sequences admitted")?,
        "identical sequences must compare equal",
    );
    ensure!(
        key_bytes(&ascending)? != key_bytes(&descending)?,
        "reordered sequences must not share a key",
    );
    Ok(())
}
