//! Boundary cases: every refusal the domain defines.
//!
//! Each case asserts the typed variant and the kind it names, so a change that
//! quietly admits an out-of-domain value fails here. The cases that compare
//! the adapter's bytes against `serde_json_canonicalizer` called directly —
//! so the adapter is never its own oracle — live in the sibling `parity`
//! battery, which is a different kind of judgement from a refusal.

use std::sync::atomic::Ordering;

use anyhow::{Result, ensure};
use minijinja::value::{Value, ValueKind};
use rstest::rstest;

use super::*;

/// Verify values outside the canonical-JSON domain are refused with a typed
/// error naming the offending kind, before any coercion.
#[rstest]
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable fixture must be visible in the captured test output instead of passing silently"
)]
fn unsupported_values_are_refused_by_kind() -> Result<()> {
    let callable = Value::from_function(|| 0);
    let invalid = Value::from_serialize(AlwaysFails);

    ensure_refused(&Value::UNDEFINED, ValueKind::Undefined, "undefined")?;
    ensure_refused(&invalid, ValueKind::Invalid, "a failed serialization")?;
    ensure_refused(&Value::from_bytes(vec![1, 2]), ValueKind::Bytes, "bytes")?;
    ensure_refused(&callable, ValueKind::Plain, "a callable")?;
    ensure_refused(
        &Value::from_object(TestIterable),
        ValueKind::Iterable,
        "a keyless iterable",
    )?;

    if let Some(now) = now_object() {
        // Measured as a plain object, not an iterable one: `net::now()` is
        // registered as a function carrying a clock, so the value it returns
        // reports `Plain`. The property under test is that it is refused at
        // all, and the kind is asserted from measurement rather than from the
        // shape the RFC text suggests.
        ensure_refused(&now, ValueKind::Plain, "the now() object")?;
    } else {
        // Say so rather than passing silently: a green run that quietly
        // skipped its subject would hide the same regression on a host that
        // can build the object.
        eprintln!("skipped: this host cannot build a now() object");
    }
    Ok(())
}

/// Verify the primitive conversions' boolean arm cannot be reached by an
/// integer, which is what the integer/binary64 split in the adapter exists for.
///
/// Both `true` and `1` report [`ValueKind::Number`] to an ungated integer
/// read, so this is the case that fails if the `is_integer` gate is removed.
#[rstest]
fn booleans_are_not_numbers() -> Result<()> {
    ensure!(
        key_bytes(&Value::from(true))? != key_bytes(&Value::from(1))?,
        "`true` must not derive the key of `1`",
    );
    ensure!(
        key_bytes(&Value::from(false))? != key_bytes(&Value::from(0))?,
        "`false` must not derive the key of `0`",
    );
    ensure!(
        key_bytes(&Value::from(true))? == b"true",
        "a boolean must canonicalize as a boolean",
    );
    ensure!(
        key_bytes(&Value::from(1))? == b"1",
        "an integer must canonicalize as an integer",
    );
    Ok(())
}

/// Verify non-finite floats are refused, at the root and below it.
#[rstest]
fn non_finite_numbers_are_refused_at_any_depth() -> Result<()> {
    for non_finite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let refusal = canonical_key(&Value::from(non_finite)).expect_err("non-finite is refused");
        ensure!(
            matches!(refusal, CanonicalValueError::NonFiniteNumber { .. }),
            "expected a non-finite refusal, got {refusal}",
        );
        ensure!(
            refusal.kind().is_none(),
            "a non-finite refusal names no value kind",
        );
        ensure!(
            refusal.to_string().contains("non-finite"),
            "the refusal must say what it refused: {refusal}",
        );
    }

    let nested = nested_map(Value::from(f64::NAN));
    let refusal = canonical_key(&nested).expect_err("a nested non-finite float is refused");
    ensure!(
        matches!(refusal, CanonicalValueError::NonFiniteNumber { .. }),
        "expected a nested non-finite refusal, got {refusal}",
    );
    ensure!(
        refusal.to_string().contains("nested"),
        "the refusal must locate the nested value: {refusal}",
    );
    Ok(())
}

/// Verify a mapping key that is not a string is refused at any depth, before
/// any coercion could let it collide with its quoted spelling.
#[rstest]
fn non_string_keys_are_refused_at_any_depth() -> Result<()> {
    let (top, _) = fixture(
        ObjectRepr::Map,
        Some(1),
        Materialized::Pairs(vec![(Value::from(1), Value::from("one"))]),
    );
    let refusal = canonical_key(&top).expect_err("an integer key is refused");
    ensure!(
        matches!(refusal, CanonicalValueError::NonStringKey { .. }),
        "expected a non-string-key refusal, got {refusal}",
    );
    ensure!(
        refusal.kind() == Some(ValueKind::Number),
        "the refusal must name the key's kind",
    );

    let (inner, _) = fixture(
        ObjectRepr::Map,
        Some(1),
        Materialized::Pairs(vec![(Value::from(true), Value::from("yes"))]),
    );
    let nested_refusal =
        canonical_key(&nested_map(inner)).expect_err("a nested boolean key is refused");
    ensure!(
        nested_refusal.kind() == Some(ValueKind::Bool),
        "the nested refusal must name the boolean kind, got {nested_refusal}",
    );
    Ok(())
}

/// Verify a repeated member name has no canonical form, a map that cannot be
/// traversed is refused rather than treated as empty, and a genuinely empty
/// map still canonicalizes.
#[rstest]
fn untraversable_and_duplicated_maps_are_refused() -> Result<()> {
    let (repeated, _) = fixture(
        ObjectRepr::Map,
        Some(2),
        Materialized::Pairs(vec![
            (Value::from("a"), Value::from(1)),
            (Value::from("a"), Value::from(2)),
        ]),
    );
    let refusal = canonical_key(&repeated).expect_err("a repeated name is refused");
    ensure!(
        matches!(
            refusal,
            CanonicalValueError::IncompatibleRepresentation { .. }
        ),
        "expected an untraversable refusal, got {refusal}",
    );

    let (empty, _) = fixture(ObjectRepr::Map, None, Materialized::NonEnumerable);
    ensure!(
        canonical_key(&empty).is_err(),
        "a non-enumerable map must be refused, not treated as empty",
    );
    ensure!(
        matches!(
            canonical_key(&empty).expect_err("still refused"),
            CanonicalValueError::IncompatibleRepresentation { .. }
        ),
        "a non-enumerable map is an untraversable representation",
    );

    let empty_map = Value::from(std::collections::BTreeMap::<String, Value>::new());
    ensure!(
        key_bytes(&empty_map)? == b"{}",
        "a genuinely empty mapping must canonicalize to {{}}",
    );
    Ok(())
}

/// Verify a host object is traversed once per key derivation, so no snapshot
/// can mix two readings of a value that enumerates differently each time.
///
/// A well-behaved custom object is admitted alongside the renewed one, so this
/// case shows the adapter accepts a host sequence that is not built in.
#[rstest]
fn a_host_object_is_enumerated_once_per_derivation() -> Result<()> {
    let (steady, calls) = fixture(
        ObjectRepr::Seq,
        Some(2),
        Materialized::Values(vec![Value::from("a"), Value::from(true)]),
    );
    ensure!(
        key_bytes(&steady)? == b"[\"a\",true]",
        "a stable custom sequence must be admitted",
    );
    ensure!(
        calls.load(Ordering::SeqCst) == 1,
        "a steady object must still be read exactly once, saw {}",
        calls.load(Ordering::SeqCst),
    );

    let (renewed, renewals) = fixture(ObjectRepr::Seq, Some(3), Materialized::Renewed { count: 3 });
    let first = key_bytes(&renewed)?;
    ensure!(
        renewals.load(Ordering::SeqCst) == 1,
        "one derivation must enumerate the host exactly once, saw {}",
        renewals.load(Ordering::SeqCst),
    );
    let second = key_bytes(&renewed)?;
    ensure!(
        renewals.load(Ordering::SeqCst) == 2,
        "a second derivation must add exactly one more enumeration, saw {}",
        renewals.load(Ordering::SeqCst),
    );
    ensure!(
        first == b"[1,2,3]",
        "the snapshot must keep the values the host yielded, got {}",
        String::from_utf8_lossy(&first),
    );
    ensure!(first == second, "a stable host object must derive one key");
    Ok(())
}

/// Verify a length that disagrees with what the host yields is refused.
#[rstest]
fn a_length_that_disagrees_with_the_enumeration_is_refused() -> Result<()> {
    let (lying, _) = fixture(ObjectRepr::Seq, Some(5), Materialized::Seeded);
    let refusal = canonical_key(&lying).expect_err("a lying length is refused");
    ensure!(
        matches!(
            refusal,
            CanonicalValueError::IncompatibleRepresentation { .. }
        ),
        "a length that disagrees with the enumeration must be refused, got {refusal}",
    );
    Ok(())
}

/// Verify the earliest refusal stops deduplication, that no partial result is
/// returned, and that a refused value compared with itself is an error rather
/// than `false`.
#[rstest]
fn refusal_stops_deduplication_and_is_never_false() -> Result<()> {
    let input = [
        Value::from("ok"),
        Value::from_bytes(vec![1]),
        Value::UNDEFINED,
    ];
    let refusal = stable_deduplicate(&input).expect_err("bytes are refused");
    ensure!(
        refusal.kind() == Some(ValueKind::Bytes),
        "the earliest refusal must be reported, got {refusal}",
    );

    let refused = Value::from_bytes(vec![7]);
    ensure!(
        canonical_eq(&refused, &refused).is_err(),
        "a refused value compared with itself must be an error, never false",
    );
    ensure!(
        canonical_key(&refused).is_err(),
        "a refused value must have no key at all",
    );
    Ok(())
}
