//! Bounded admission: exact ceilings, refusal precedence, and operation scope.
//!
//! A `MiniJinja` value may be a directed acyclic graph, because cloning a value
//! shares its host object. Admission materializes one snapshot subtree per
//! occurrence, so a compact graph costs work proportional to its *expansion* —
//! a depth-64 chain of two-item sequences is 65 objects but `2^65 - 1`
//! occurrences. This battery pins the bounds that keep that work finite: the
//! exact boundary of each ceiling, the precedence admission keeps while
//! charging, and the fact that one budget serves one public operation rather
//! than one value.
//!
//! Cases drive either the public surface or the crate-internal entry that takes
//! an explicit budget, so a refusal reaches the real adapter and an admission
//! reaches the real RFC 8785 canonicalizer.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, ensure};
use minijinja::value::{ObjectRepr, Value};
use rstest::rstest;

use super::*;

use crate::stdlib::canonical_value::budget::AdmissionLimits;
use crate::stdlib::canonical_value::error::SnapshotPath;

/// Depth of the shared graph that exceeds every default ceiling.
///
/// Its expanded occurrence count is `2^65 - 1`, which no host could allocate,
/// so the case can pass only by refusing before expansion rather than by
/// completing it.
const EXPONENTIAL_DEPTH: usize = 64;

/// Build a sequence fixture that can yield `bound` items, counting each pull.
fn counted_sequence(bound: usize) -> (Value, Arc<AtomicUsize>) {
    let yielded = Arc::new(AtomicUsize::new(0));
    let (value, _) = fixture(
        ObjectRepr::Seq,
        Some(bound),
        Materialized::Counted {
            bound,
            yielded: Arc::clone(&yielded),
        },
    );
    (value, yielded)
}

/// Build a map fixture from literal pairs, reporting `len` as its length.
fn pair_map(pairs: Vec<(Value, Value)>, len: Option<usize>) -> Value {
    fixture(ObjectRepr::Map, len, Materialized::Pairs(pairs)).0
}

/// Assert the recorded enumerations total no more than `limit`.
fn ensure_enumeration_bounded(counters: &[Arc<AtomicUsize>], limit: usize) -> Result<()> {
    let observed: usize = counters
        .iter()
        .map(|calls| calls.load(Ordering::SeqCst))
        .sum();
    ensure!(
        observed <= limit,
        "admission enumerated {observed} occurrences, beyond the node ceiling of {limit}",
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The reported trigger
// ---------------------------------------------------------------------------

/// A depth-64 shared graph is refused before it is expanded.
#[rstest]
fn canonical_key_refuses_an_exponentially_shared_graph() -> Result<()> {
    let (graph, counters) = shared_sequence(EXPONENTIAL_DEPTH);
    let refusal = canonical_key(&graph).expect_err("2^65 occurrences cannot be admitted");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    ensure_enumeration_bounded(&counters, AdmissionLimits::default().nodes)
}

/// The same graph is refused as the right operand of `canonical_eq`.
#[rstest]
fn canonical_eq_refuses_an_exponentially_shared_right_operand() -> Result<()> {
    let (graph, counters) = shared_sequence(EXPONENTIAL_DEPTH);
    let refusal =
        canonical_eq(&Value::from(1), &graph).expect_err("the shared operand cannot be admitted");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    ensure_enumeration_bounded(&counters, AdmissionLimits::default().nodes)
}

/// The same graph is refused after valid inputs in `stable_deduplicate`.
#[rstest]
fn stable_deduplicate_refuses_an_exponentially_shared_input() -> Result<()> {
    let (graph, counters) = shared_sequence(EXPONENTIAL_DEPTH);
    let input = [Value::from("first"), Value::from(2), graph];
    let refusal =
        stable_deduplicate(&input).expect_err("an over-budget input produces no partial result");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    ensure_enumeration_bounded(&counters, AdmissionLimits::default().nodes)
}

// ---------------------------------------------------------------------------
// Exact ceilings
// ---------------------------------------------------------------------------

/// A value exactly at the depth ceiling is admitted; one edge deeper is not.
#[rstest]
#[case::at_the_ceiling(1, true)]
#[case::one_edge_past(0, false)]
fn depth_ceiling_is_inclusive(#[case] max_depth: usize, #[case] admits: bool) -> Result<()> {
    let nested = Value::from([Value::from(1_i64)]);
    let admitted =
        canonical_key_within(&nested, &mut admission_budget(max_depth, 100, 100)).is_ok();
    ensure!(
        admitted == admits,
        "max_depth={max_depth} admitted={admitted}",
    );
    Ok(())
}

/// A sequence exactly at the node ceiling is admitted; one node past is not.
#[rstest]
#[case::at_the_ceiling(4, true)]
#[case::one_node_past(3, false)]
fn node_ceiling_is_inclusive(#[case] max_nodes: usize, #[case] admits: bool) -> Result<()> {
    let flat = Value::from([1_i64, 2, 3]);
    let admitted = canonical_key_within(&flat, &mut admission_budget(100, max_nodes, 100)).is_ok();
    ensure!(
        admitted == admits,
        "max_nodes={max_nodes} admitted={admitted}",
    );
    Ok(())
}

/// A string exactly at the byte ceiling is admitted; one byte past is not.
#[rstest]
#[case::at_the_ceiling(3, true)]
#[case::one_byte_past(2, false)]
fn string_byte_ceiling_is_inclusive(#[case] max_bytes: usize, #[case] admits: bool) -> Result<()> {
    let text = Value::from("abc");
    let admitted = canonical_key_within(&text, &mut admission_budget(100, 100, max_bytes)).is_ok();
    ensure!(
        admitted == admits,
        "max_bytes={max_bytes} admitted={admitted}",
    );
    Ok(())
}

/// A member name exactly at the byte ceiling is admitted; one byte past is not.
#[rstest]
#[case::at_the_ceiling(3, true)]
#[case::one_byte_past(2, false)]
fn member_name_byte_ceiling_is_inclusive(
    #[case] max_bytes: usize,
    #[case] admits: bool,
) -> Result<()> {
    let map = map_of(vec![("abc".to_owned(), Value::from(1_i64))]);
    let admitted = canonical_key_within(&map, &mut admission_budget(100, 100, max_bytes)).is_ok();
    ensure!(
        admitted == admits,
        "max_bytes={max_bytes} admitted={admitted}",
    );
    Ok(())
}

/// A depth refusal names the first value past the ceiling, not the root.
#[rstest]
fn depth_refusal_names_the_first_value_past_the_ceiling() -> Result<()> {
    let nested = Value::from([Value::from(1_i64)]);
    let refusal = canonical_key_within(&nested, &mut admission_budget(0, 100, 100))
        .expect_err("the item is one edge past a zero-depth ceiling");
    ensure!(
        refusal.kind().is_none(),
        "a resource refusal names no value kind, got {refusal}",
    );
    let CanonicalValueError::ResourceLimitExceeded {
        resource,
        limit,
        path,
    } = refusal
    else {
        anyhow::bail!("expected a resource-limit refusal");
    };
    ensure!(
        resource == CanonicalResource::Depth,
        "expected the depth ceiling, got {resource}",
    );
    ensure!(limit == 0, "the refusal must report the configured ceiling");
    ensure!(
        path == "[0]",
        "the refusal must name the refused item, got {path}",
    );
    Ok(())
}

/// An over-long sequence stops one pull past the remaining node allowance.
#[rstest]
fn oversized_sequence_stops_one_item_past_the_allowance() -> Result<()> {
    let (sequence, yielded) = counted_sequence(1_000_000);
    let refusal = canonical_key_within(&sequence, &mut admission_budget(100, 4, 100))
        .expect_err("a million items exceed a four-node allowance");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    // The root charges one node, leaving three; one pull beyond that allowance
    // is what distinguishes a refused overflow from a silently truncated read.
    let pulled = yielded.load(Ordering::SeqCst);
    ensure!(
        pulled == 4,
        "the enumeration must stop after four pulls, got {pulled}",
    );
    Ok(())
}

/// A shared graph and its independent expansion have the same canonical key.
#[rstest]
fn shared_graph_matches_its_expanded_tree() -> Result<()> {
    let (shared, _) = shared_sequence(4);
    let expanded = expanded_sequence(4);
    ensure!(
        canonical_key(&shared)? == canonical_key(&expanded)?,
        "sharing must not change the canonical key",
    );
    ensure!(
        canonical_eq(&shared, &expanded)?,
        "the equality relation must agree with the key comparison",
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Operation scope
// ---------------------------------------------------------------------------

/// `canonical_eq` spends one budget across both operands.
#[rstest]
fn canonical_eq_shares_one_budget_across_operands() -> Result<()> {
    let left = Value::from([1_i64, 2]);
    let right = Value::from([1_i64, 2]);
    ensure!(
        canonical_key_within(&left, &mut admission_budget(100, 4, 100)).is_ok(),
        "each operand must fit the ceiling on its own",
    );
    ensure!(
        canonical_key_within(&right, &mut admission_budget(100, 4, 100)).is_ok(),
        "each operand must fit the ceiling on its own",
    );
    let refusal = canonical_eq_within(&left, &right, &mut admission_budget(100, 4, 100))
        .expect_err("one budget cannot admit both operands");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    Ok(())
}

/// `stable_deduplicate` spends one budget across every input.
#[rstest]
fn stable_deduplicate_shares_one_budget_across_inputs() -> Result<()> {
    let first = Value::from([1_i64, 2]);
    let second = Value::from([1_i64, 2]);
    ensure!(
        canonical_key_within(&first, &mut admission_budget(100, 4, 100)).is_ok(),
        "each input must fit the ceiling on its own",
    );
    ensure!(
        canonical_key_within(&second, &mut admission_budget(100, 4, 100)).is_ok(),
        "each input must fit the ceiling on its own",
    );
    let input = [first, second];
    let refusal = stable_deduplicate_within(&input, &mut admission_budget(100, 4, 100))
        .expect_err("one budget cannot admit both inputs");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Refusal precedence
// ---------------------------------------------------------------------------

/// A non-string key is refused before any member value is charged.
#[rstest]
fn non_string_key_precedes_the_member_byte_ceiling() -> Result<()> {
    let map = pair_map(
        vec![(Value::from(1_i64), Value::from("x".repeat(100)))],
        Some(1),
    );
    let refusal = canonical_key_within(&map, &mut admission_budget(100, 100, 0))
        .expect_err("an integer key is refused");
    ensure!(
        matches!(refusal, CanonicalValueError::NonStringKey { .. }),
        "the key check must precede byte charging, got {refusal}",
    );
    Ok(())
}

/// A repeated member name is refused before any child value is admitted.
#[rstest]
fn duplicate_name_precedes_the_child_depth_ceiling() -> Result<()> {
    let name = "a".to_owned();
    let map = pair_map(
        vec![
            (Value::from(name.clone()), Value::from(1_i64)),
            (Value::from(name), Value::from(2_i64)),
        ],
        Some(2),
    );
    let refusal = canonical_key_within(&map, &mut admission_budget(0, 100, 100))
        .expect_err("a repeated member name has no canonical form");
    ensure!(
        matches!(
            refusal,
            CanonicalValueError::IncompatibleRepresentation { .. }
        ),
        "the duplicate check must precede child admission, got {refusal}",
    );
    Ok(())
}

/// A pair overflow is refused before the reported length is compared.
#[rstest]
fn pair_overflow_precedes_the_length_mismatch() -> Result<()> {
    let pairs: Vec<(Value, Value)> = (0..100)
        .map(|index| {
            (
                Value::from(format!("k{index:03}")),
                Value::from(i64::from(index)),
            )
        })
        .collect();
    let map = pair_map(pairs, Some(1));

    let overflow = canonical_key_within(&map, &mut admission_budget(100, 2, 1_000))
        .expect_err("the pair count exceeds a two-node allowance");
    ensure_resource_refusal(&overflow, CanonicalResource::Nodes)?;

    // With enough nodes the same map reaches the length check, which is what
    // shows the overflow above was decided by the budget and not by the
    // mismatch.
    let mismatch = canonical_key_within(&map, &mut admission_budget(100, 300, 1_000))
        .expect_err("the reported length disagrees with the enumeration");
    ensure!(
        matches!(
            mismatch,
            CanonicalValueError::IncompatibleRepresentation { .. }
        ),
        "expected a length refusal once the nodes fit, got {mismatch}",
    );
    Ok(())
}

/// A refused charge leaves the allowance it failed to consume untouched.
///
/// The check that follows the refusal is what makes this discriminating. A
/// refusal only happens once the allowance is already spent, so re-reading the
/// remainder afterwards cannot distinguish an untouched counter from an
/// incremented one; a later charge that fits only if the refused one was
/// discarded can.
#[rstest]
fn an_over_budget_charge_does_not_consume_the_allowance() -> Result<()> {
    let path = SnapshotPath::root();
    let mut account = admission_budget(usize::MAX, 100, 3);
    account.charge_bytes(2, &path).expect("two bytes fit");
    let refusal = account
        .charge_bytes(2, &path)
        .expect_err("a second two-byte charge exceeds a three-byte ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Bytes)?;
    account
        .charge_bytes(1, &path)
        .expect("the refused charge must leave the remaining byte allowance intact");
    Ok(())
}
