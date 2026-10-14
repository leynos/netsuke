//! Host traversal: what the adapter requires of a custom `MiniJinja` object.
//!
//! These cases are separated from the refusals in the sibling `boundaries`
//! battery because they make a different kind of judgement. A boundary case
//! asks whether an out-of-domain value is turned away; these ask whether an
//! in-domain host object is read the way the documented precondition requires —
//! exactly once per derivation, so a snapshot cannot mix two readings of a
//! value that would enumerate differently the second time. Both are contract
//! checks, but only the second is about the purity of a single traversal, and
//! `admission` states that contract in prose.
//!
//! The counter each case reads is the evidence: it is what turns "the adapter
//! reads the host once" from a claim about the source into an observation.

use std::sync::atomic::Ordering;

use anyhow::{Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::*;

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
        "the snapshot must keep the values the host yielded on its first \
         enumeration, got {}",
        String::from_utf8_lossy(&first),
    );
    // The second derivation must reflect its own enumeration. An adapter that
    // re-read the host while serializing the first snapshot would report
    // `[2,3,4]` here, and one that cached the snapshot would report `[1,2,3]`.
    ensure!(
        second == b"[2,3,4]",
        "each derivation must read the host afresh, got {}",
        String::from_utf8_lossy(&second),
    );
    ensure!(
        first != second,
        "a host whose content changes must not derive one key for two readings",
    );
    Ok(())
}

/// Verify a length that disagrees with what the host yields is refused.
///
/// Both representations are checked, because each is collected by its own
/// function: a stability check written into one collector leaves the other
/// admitting a host object it cannot snapshot reproducibly.
#[rstest]
fn a_length_that_disagrees_with_the_enumeration_is_refused() -> Result<()> {
    let lying = [
        (ObjectRepr::Seq, Some(5), Materialized::Seeded),
        (
            ObjectRepr::Map,
            Some(1),
            Materialized::Pairs(vec![
                (Value::from("a"), Value::from(1)),
                (Value::from("b"), Value::from(2)),
            ]),
        ),
    ];
    for (repr, len, materialized) in lying {
        let (probe, _) = fixture(repr, len, materialized);
        let refusal = canonical_key(&probe).expect_err("a lying length is refused");
        ensure!(
            matches!(
                refusal,
                CanonicalValueError::IncompatibleRepresentation { .. }
            ),
            "a {repr:?} length that disagrees with the enumeration must be refused, got {refusal}",
        );
    }
    Ok(())
}
