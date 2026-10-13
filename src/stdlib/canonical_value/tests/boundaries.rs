//! Boundary cases: every refusal the domain defines.
//!
//! Each case asserts the typed variant and the kind it names, so a change that
//! quietly admits an out-of-domain value fails here. The cases that compare
//! the adapter's bytes against `serde_json_canonicalizer` called directly —
//! so the adapter is never its own oracle — live in the sibling `parity`
//! battery, which is a different kind of judgement from a refusal.

use anyhow::{Result, ensure};
use minijinja::value::{Value, ValueKind};
use rstest::rstest;

use super::*;

/// Verify values outside the canonical-JSON domain are refused with a typed
/// error naming the offending kind, before any coercion.
#[rstest]
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

    // The refusal is part of the documented domain, so it is asserted rather
    // than probed: a host where the fixture cannot be built reports its error
    // here instead of leaving the case quietly untested. The object is measured
    // as a plain one, not an iterable one — `now()` is registered as a function
    // carrying a clock, so the value it returns reports `Plain`. The kind is
    // asserted from that measurement rather than from the shape the RFC text
    // suggests.
    let now = now_object()?;
    ensure_refused(&now, ValueKind::Plain, "the now() object")?;

    ensure_refused(&nested_map(now), ValueKind::Plain, "a nested now() object")?;
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

/// Verify a refusal is the same whichever order the host enumerates.
///
/// Two maps built from the same members in opposite orders must produce the
/// same refusal, because the canonical form of a map is content-defined and a
/// host's enumeration order is not part of the value. The kinds are also
/// asserted explicitly, so a fixture that stopped discriminating would fail
/// rather than agreeing with a degenerate expectation.
#[rstest]
fn refusals_are_independent_of_host_enumeration_order() -> Result<()> {
    // Several non-string keys in one map: the lowest kind must decide, in
    // either order. `Bool` sorts below `Number`, and `Bytes` below `Seq`.
    let boolean_and_number = [
        (Value::from(true), Value::from("yes")),
        (Value::from(1), Value::from("one")),
    ];
    for members in [boolean_and_number.to_vec(), {
        let mut reversed = boolean_and_number.to_vec();
        reversed.reverse();
        reversed
    }] {
        let (probe, _) = fixture(ObjectRepr::Map, Some(2), Materialized::Pairs(members));
        let refusal = canonical_key(&probe).expect_err("a non-string key is refused");
        ensure!(
            refusal.kind() == Some(ValueKind::Bool),
            "the lowest non-string kind must be reported in either order, got {refusal}",
        );
    }

    let bytes_and_seq = [
        (Value::from_bytes(vec![1]), Value::from("bytes key")),
        (Value::from(vec![1]), Value::from("seq key")),
    ];
    for members in [bytes_and_seq.to_vec(), {
        let mut reversed = bytes_and_seq.to_vec();
        reversed.reverse();
        reversed
    }] {
        let (probe, _) = fixture(ObjectRepr::Map, Some(2), Materialized::Pairs(members));
        let refusal = canonical_key(&probe).expect_err("a non-string key is refused");
        ensure!(
            refusal.kind() == Some(ValueKind::Bytes),
            "the lowest non-string kind must be reported in either order, got {refusal}",
        );
    }

    // Two inadmissible *values* under admitted keys: the refusal must come from
    // the first key the canonical order visits, not from the first enumerated.
    let invalid_values = [
        (Value::from("b"), Value::UNDEFINED),
        (Value::from("a"), Value::from_bytes(vec![1])),
    ];
    for members in [invalid_values.to_vec(), {
        let mut reversed = invalid_values.to_vec();
        reversed.reverse();
        reversed
    }] {
        let (probe, _) = fixture(ObjectRepr::Map, Some(2), Materialized::Pairs(members));
        let refusal = canonical_key(&probe).expect_err("one member value is refused");
        ensure!(
            refusal.kind() == Some(ValueKind::Bytes),
            "the refusal must come from the lowest member name, got {refusal}",
        );
    }
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

    // Both representations that an empty enumeration could be mistaken for an
    // empty collection are checked, because the refusal is reached through a
    // different collector for each.
    for repr in [ObjectRepr::Map, ObjectRepr::Seq] {
        let (unreachable, _) = fixture(repr, None, Materialized::NonEnumerable);
        ensure!(
            canonical_key(&unreachable).is_err(),
            "a non-enumerable {repr:?} must be refused, not treated as empty",
        );
        ensure!(
            matches!(
                canonical_key(&unreachable).expect_err("still refused"),
                CanonicalValueError::IncompatibleRepresentation { .. }
            ),
            "a non-enumerable {repr:?} is an untraversable representation",
        );
    }

    let empty_map = Value::from(std::collections::BTreeMap::<String, Value>::new());
    ensure!(
        key_bytes(&empty_map)? == b"{}",
        "a genuinely empty mapping must canonicalize to {{}}",
    );
    Ok(())
}

/// Verify `canonical_eq` admits both operands before it compares them.
///
/// The relation is byte-identity over canonical keys, so an operand outside the
/// domain has no answer at all. A short-circuit that returned `false` on the
/// first unadmitted operand would let a caller believe two values had been
/// compared. The documented rule for two invalid operands is that the refusal
/// names the left one, which is what makes the error reproducible rather than
/// dependent on argument order in the caller's source.
#[rstest]
fn equality_admits_both_operands_before_comparing() -> Result<()> {
    let valid = Value::from(1);
    let bytes = Value::from_bytes(vec![7]);
    let undefined = Value::UNDEFINED;

    ensure!(
        canonical_eq(&valid, &valid).context("an admitted operand is comparable")?,
        "an admitted value must equal itself",
    );

    let right_refusal =
        canonical_eq(&valid, &bytes).expect_err("the right operand is not admitted");
    ensure!(
        right_refusal.kind() == Some(ValueKind::Bytes),
        "a valid left operand must not mask an invalid right operand, got {right_refusal}",
    );
    ensure!(
        canonical_eq(&bytes, &valid).is_err(),
        "an invalid left operand must not be answered `false`",
    );

    let left_refusal = canonical_eq(&bytes, &undefined).expect_err("both operands are refused");
    ensure!(
        left_refusal.kind() == Some(ValueKind::Bytes),
        "the left operand's refusal must be reported when both are invalid, got {left_refusal}",
    );
    ensure!(
        left_refusal.kind()
            != canonical_eq(&undefined, &bytes)
                .expect_err("still refused")
                .kind(),
        "swapping two invalid operands must swap the refusal",
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
