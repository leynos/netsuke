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

use crate::stdlib::canonical_value::budget::{AdmissionBudget, AdmissionLimits};
use crate::stdlib::canonical_value::path::PathRef;

/// Depth of the shared graph that exceeds every default ceiling.
///
/// Its expanded occurrence count is `2^65 - 1`, which no host could allocate,
/// so the case can pass only by refusing before expansion rather than by
/// completing it.
const EXPONENTIAL_DEPTH: usize = 64;

/// Build a sequence fixture that can yield `bound` items, counting each pull.
///
/// `path_bookkeeping` builds its wide leaf from this fixture too, so the pull
/// counter it asserts against is the same one the ceiling cases use.
pub(super) fn counted_sequence(bound: usize) -> (Value, Arc<AtomicUsize>) {
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
///
/// Shared with `path_bookkeeping`, which stacks long-named mappings above the
/// wide leaf.
pub(super) fn pair_map(pairs: Vec<(Value, Value)>, len: Option<usize>) -> Value {
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

/// The ceiling a case exercises, named by what it meters.
#[derive(Clone, Copy, Debug)]
enum Ceiling {
    /// Distance in edges below the snapshot root.
    Depth,
    /// Snapshot nodes, counted per occurrence.
    Nodes,
    /// Bytes of an admitted scalar.
    Bytes,
    /// Bytes of a mapping member name.
    MemberNameBytes,
}

impl Ceiling {
    /// Return the resource this ceiling reports when it refuses.
    const fn resource(self) -> CanonicalResource {
        match self {
            Self::Depth => CanonicalResource::Depth,
            Self::Nodes => CanonicalResource::Nodes,
            Self::Bytes | Self::MemberNameBytes => CanonicalResource::Bytes,
        }
    }
}

/// Build a fixture that spends exactly `allowance` of `ceiling`.
///
/// Every other ceiling is set generously, so the fixture costs the named one
/// and nothing else; passing `allowance - 1` yields the same value one unit
/// over budget.
fn at_ceiling(ceiling: Ceiling, allowance: usize) -> (Value, AdmissionBudget) {
    match ceiling {
        Ceiling::Depth => (
            Value::from([Value::from(1_i64)]),
            admission_budget(allowance, 100, 100),
        ),
        Ceiling::Nodes => (
            Value::from([1_i64, 2, 3]),
            admission_budget(100, allowance, 100),
        ),
        Ceiling::Bytes => (Value::from("abc"), admission_budget(100, 100, allowance)),
        Ceiling::MemberNameBytes => (
            map_of(vec![("abc".to_owned(), Value::from(1_i64))]),
            admission_budget(100, 100, allowance),
        ),
    }
}

/// Each ceiling admits its exact allowance and refuses one unit less.
#[rstest]
#[case::depth(Ceiling::Depth, 1)]
#[case::nodes(Ceiling::Nodes, 4)]
#[case::text_bytes(Ceiling::Bytes, 3)]
#[case::member_name_bytes(Ceiling::MemberNameBytes, 3)]
fn ceiling_is_inclusive(#[case] ceiling: Ceiling, #[case] exact: usize) -> Result<()> {
    let (at_limit, mut at_limit_budget) = at_ceiling(ceiling, exact);
    ensure!(
        canonical_key_within(&at_limit, &mut at_limit_budget).is_ok(),
        "a value that spends exactly the {ceiling:?} ceiling must be admitted",
    );
    let (past_limit, mut past_limit_budget) = at_ceiling(ceiling, exact - 1);
    let refusal = canonical_key_within(&past_limit, &mut past_limit_budget)
        .expect_err("a value one unit past the ceiling must be refused");
    ensure_resource_refusal(&refusal, ceiling.resource())
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

/// A container one edge past the depth ceiling is refused before enumerating.
///
/// The counted sequence is the value the ceiling refuses, not an ancestor of
/// it, so a traversal that checked depth only after dispatching into a
/// container would pull at least once before refusing and fail the count.
#[rstest]
fn a_container_past_the_depth_ceiling_is_refused_before_it_enumerates() -> Result<()> {
    let (counted, pulls) = counted_sequence(10);
    let nested = Value::from(vec![Value::from(vec![counted])]);
    let refusal = canonical_key_within(&nested, &mut admission_budget(1, 100, 100))
        .expect_err("the counted sequence sits two edges below a one-edge ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Depth)?;
    let pulled = pulls.load(Ordering::SeqCst);
    ensure!(
        pulled == 0,
        "the refused container must not be enumerated, got {pulled} pulls",
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
// Refusal precedence
// ---------------------------------------------------------------------------

/// A non-string key is refused before any member value is charged.
///
/// The value behind the key is larger than the whole byte ceiling, so an
/// implementation that admitted or charged values before checking keys would
/// refuse `Bytes`; the expected `NonStringKey` is what shows the key check ran
/// first. A non-string key has no member-name charge of its own, which is why
/// the ceiling this test orders against is the value's rather than a name's.
#[rstest]
fn non_string_key_precedes_the_value_byte_ceiling() -> Result<()> {
    let map = pair_map(
        vec![(Value::from(1_i64), Value::from("x".repeat(100)))],
        Some(1),
    );
    let refusal = canonical_key_within(&map, &mut admission_budget(100, 100, 0))
        .expect_err("an integer key is refused");
    ensure!(
        matches!(refusal, CanonicalValueError::NonStringKey { .. }),
        "the key check must precede value byte charging, got {refusal}",
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
    let path = PathRef::root();
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

// The cost of tracking a snapshot location is a separate claim from the size
// of the value being admitted, so those cases live in `path_bookkeeping`.
