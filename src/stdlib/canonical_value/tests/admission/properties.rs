//! Property tests for bounded canonical admission.
//!
//! The `budget` battery pins the exact boundary of each ceiling and the
//! reported trigger at one depth with hand-picked fixtures. This battery makes
//! the same claims over generated shapes: a shared chain of random depth and
//! leaf text, a generated pair of node and byte ceilings, and generated charge
//! sequences against one budget.
//!
//! The chain is the reported defect's simplest witness. Each level holds two
//! clones of one child, so `d` containers are `2^(d + 1) - 1` occurrences, and
//! the generated depth stays small enough that every case lands inside the
//! production node ceiling except where a generated ceiling says otherwise.
//!
//! The generator lives here rather than in the `properties` battery beside
//! this directory module because it models admission's charge model rather
//! than a general value shape: the depth and leaf text are chosen so the
//! occurrence and byte totals are derivable, which a shape generator for
//! canonical keys does not need.

use anyhow::Result;
use minijinja::value::Value;
use proptest::prelude::*;

use super::super::*;

use super::super::properties::law_runner;
use crate::stdlib::canonical_value::path::PathRef;

/// Deepest generated chain, counted in containers.
///
/// Seven containers expand to 127 occurrences, which the production node
/// ceiling admits; a deeper chain would cross it and stop exercising the
/// admitted half of the refusal property.
const MAX_GENERATED_DEPTH: u32 = 6;

/// One generated shared chain, its expansion, and the charges each requests.
#[derive(Debug)]
struct SharedChain {
    /// The chain whose children are clones of one host object.
    shared: Value,
    /// The same content built with no shared objects.
    expanded: Value,
    /// Snapshot nodes the chain charges: `2^(depth + 1) - 1`.
    occurrences: usize,
    /// Bytes the chain charges: one leaf per bottom-level branch.
    leaf_bytes: usize,
}

impl SharedChain {
    /// Build the shared chain and the independently allocated tree it equals.
    fn new(depth: u32, leaf: &str) -> Self {
        let shared = (0..depth).fold(Value::from(leaf), |child, _| {
            Value::from(vec![child.clone(), child])
        });
        Self {
            shared,
            expanded: expanded_chain(depth, leaf),
            occurrences: (1usize << (depth + 1)) - 1,
            leaf_bytes: (1usize << depth) * leaf.len(),
        }
    }
}

/// Build the same chain with every occurrence a distinct host object.
fn expanded_chain(depth: u32, leaf: &str) -> Value {
    if depth == 0 {
        Value::from(leaf)
    } else {
        Value::from(vec![
            expanded_chain(depth - 1, leaf),
            expanded_chain(depth - 1, leaf),
        ])
    }
}

/// Generate a shared chain of random depth and leaf text.
fn shared_chains() -> BoxedStrategy<SharedChain> {
    (0u32..=MAX_GENERATED_DEPTH, "[a-z]{0,8}")
        .prop_map(|(depth, leaf)| SharedChain::new(depth, &leaf))
        .boxed()
}

/// One charge a generated sequence asks of a budget.
#[derive(Clone, Copy, Debug)]
enum ChargeStep {
    /// Charge one snapshot node.
    Node,
    /// Charge this many bytes of admitted text.
    Bytes(usize),
}

/// Generate one charge step.
fn charge_steps() -> impl Strategy<Value = ChargeStep> {
    prop_oneof![
        Just(ChargeStep::Node),
        (0usize..8).prop_map(ChargeStep::Bytes),
    ]
}

/// A shared chain inside every ceiling has its expansion's canonical key.
///
/// Both values are admitted by the production ceilings, so this compares two
/// successful derivations rather than asserting a refusal. Sharing must not
/// change the key, because the key is a function of content and a shared graph
/// has the same content as its expansion.
#[test]
fn generated_shared_chain_matches_its_expansion() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&shared_chains(), |chain| {
            let key_of = |value: &Value| {
                canonical_key(value).map_err(|error| TestCaseError::fail(error.to_string()))
            };
            prop_assert_eq!(
                key_of(&chain.shared)?,
                key_of(&chain.expanded)?,
                "sharing must not change the canonical key",
            );
            Ok(())
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

/// A generated ceiling refuses exactly the chains it excludes.
///
/// The expected outcome is derived from the chain's charge model rather than
/// from the admission result, so a traversal that charged a different number of
/// occurrences would disagree with the expectation. When only one ceiling is
/// exceeded its resource is asserted too; when both are, either may be reported
/// first because the charges interleave.
#[test]
fn generated_ceilings_refuse_exactly_what_they_bound() -> Result<()> {
    let mut runner = law_runner();
    let cases = (shared_chains(), 0usize..=200, 0usize..=600);
    runner
        .run(&cases, |(chain, nodes, bytes)| {
            let over_nodes = chain.occurrences > nodes;
            let over_bytes = chain.leaf_bytes > bytes;
            let mut budget = admission_budget(16, nodes, bytes);
            let outcome = canonical_key_within(&chain.shared, &mut budget);
            if !over_nodes && !over_bytes {
                prop_assert!(
                    outcome.is_ok(),
                    "a chain inside every ceiling must be admitted: {outcome:?}",
                );
                return Ok(());
            }
            let Err(refusal) = outcome else {
                return Err(TestCaseError::fail("an over-budget chain must be refused"));
            };
            if over_nodes && !over_bytes {
                prop_assert_eq!(refusal.resource(), Some(CanonicalResource::Nodes));
            }
            if over_bytes && !over_nodes {
                prop_assert_eq!(refusal.resource(), Some(CanonicalResource::Bytes));
            }
            Ok(())
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

/// A refused charge never consumes the allowance it could not spend.
///
/// Each generated sequence is replayed against one budget. A charge expected to
/// fit must succeed; one expected to overflow must be refused, and the
/// allowance it failed to spend must still be chargeable afterwards. Both
/// meters are exercised against the same ceiling so one property covers node
/// and byte accounting without duplicating the replay.
#[test]
fn refused_charges_never_consume_the_allowance() -> Result<()> {
    let mut runner = law_runner();
    let cases = (0usize..64, prop::collection::vec(charge_steps(), 1..24));
    runner
        .run(&cases, |(limit, steps)| {
            let mut budget = admission_budget(usize::MAX, limit, limit);
            let path = PathRef::root();
            let (mut used_nodes, mut used_bytes) = (0usize, 0usize);
            for step in steps {
                let (used, amount, outcome) = match step {
                    ChargeStep::Node => (&mut used_nodes, 1, budget.charge_node(&path)),
                    ChargeStep::Bytes(bytes) => {
                        (&mut used_bytes, bytes, budget.charge_bytes(bytes, &path))
                    }
                };
                let fits = used.checked_add(amount).is_some_and(|next| next <= limit);
                if fits {
                    prop_assert!(outcome.is_ok(), "a charge inside the ceiling must succeed");
                    *used += amount;
                    continue;
                }
                prop_assert!(
                    outcome.is_err(),
                    "a charge past the ceiling must be refused"
                );
                let remaining = limit - *used;
                let intact = match step {
                    ChargeStep::Node => (0..remaining)
                        .try_for_each(|_| budget.charge_node(&path))
                        .is_ok(),
                    ChargeStep::Bytes(_) => budget.charge_bytes(remaining, &path).is_ok(),
                };
                prop_assert!(
                    intact,
                    "a refused charge must leave the remaining allowance intact",
                );
                *used = limit;
            }
            Ok(())
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}
