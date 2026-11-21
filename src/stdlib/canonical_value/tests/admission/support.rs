//! Shared fixtures and assertions for bounded canonical admission.
//!
//! Several batteries need the same support: `budget` checks the exact
//! ceilings and precedence, `operation_scope` the shared budget,
//! `diagnostics` how each refusal renders, and `path_bookkeeping` the cost of
//! tracking a location. The counted fixtures live here with the rest, so a
//! battery that needs a host which reports what admitted work asked of it does
//! not build a private copy. Both need a budget built from small explicit ceilings, an assertion
//! that a refusal names a particular resource, and the shared host graph whose
//! expansion is exponential in its depth. They live here so neither battery can
//! drift onto a private copy, and the parent module re-exports them so the
//! fixtures still read as part of the shared set.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use anyhow::{Result, ensure};
use minijinja::value::{ObjectRepr, Value};

use super::super::*;

use crate::stdlib::canonical_value::budget::{AdmissionBudget, AdmissionLimits};

/// Build an admission budget from explicit ceilings.
///
/// The budget fields are crate-private, so a battery that needs a smaller
/// ceiling than production uses cannot construct one without this shim.
pub(in crate::stdlib::canonical_value) fn admission_budget(
    depth: usize,
    nodes: usize,
    bytes: usize,
) -> AdmissionBudget {
    AdmissionBudget::new(AdmissionLimits {
        depth,
        nodes,
        bytes,
    })
}

/// Assert that `refusal` names `expected` as the exhausted ceiling.
///
/// Failing with the rendered message keeps the diagnostic in the failure, so a
/// refusal for the wrong resource is not mistaken for a missing refusal.
pub(in crate::stdlib::canonical_value) fn ensure_resource_refusal(
    refusal: &CanonicalValueError,
    expected: CanonicalResource,
) -> Result<()> {
    ensure!(
        matches!(
            refusal,
            CanonicalValueError::ResourceLimitExceeded { resource, .. } if *resource == expected
        ),
        "expected a {expected} refusal, got {refusal}",
    );
    Ok(())
}

/// A counted sequence and the two kinds of work admission asked of it.
///
/// The counters are kept apart because they answer different questions. `pulls`
/// counts items yielded, so it shows how far a bounded enumeration read;
/// `enumerations` counts calls to the host's `enumerate`, so it shows whether
/// admission asked the host to enumerate at all. A container refused before
/// dispatch must leave both at zero — a traversal that returned before the
/// first pull would leave `pulls` at zero while still having called the host.
pub(in crate::stdlib::canonical_value) struct CountedSequence {
    /// The host value built from the fixture.
    pub(in crate::stdlib::canonical_value) value: Value,
    /// Items yielded so far.
    pub(in crate::stdlib::canonical_value) pulls: Arc<AtomicUsize>,
    /// Calls to the host's `enumerate` so far.
    pub(in crate::stdlib::canonical_value) enumerations: Arc<AtomicUsize>,
}

/// Build a sequence fixture that can yield `bound` items, counting the work.
///
/// `path_bookkeeping` builds its wide leaf from this fixture too, so the pull
/// counter it asserts against is the same one the ceiling cases use.
pub(in crate::stdlib::canonical_value) fn counted_sequence(bound: usize) -> CountedSequence {
    let pulls = Arc::new(AtomicUsize::new(0));
    let (value, enumerations) = fixture(
        ObjectRepr::Seq,
        Some(bound),
        Materialized::Counted {
            bound,
            yielded: Arc::clone(&pulls),
        },
    );
    CountedSequence {
        value,
        pulls,
        enumerations,
    }
}

/// Build a map fixture from literal pairs, reporting `len` as its length.
///
/// Shared with `path_bookkeeping`, which stacks long-named mappings above the
/// wide leaf.
pub(in crate::stdlib::canonical_value) fn pair_map(
    pairs: Vec<(Value, Value)>,
    len: Option<usize>,
) -> Value {
    fixture(ObjectRepr::Map, len, Materialized::Pairs(pairs)).0
}

/// Build a shared host graph of `depth` nested two-item sequences.
///
/// Every level holds two clones of the *same* child value, so the host is a
/// directed acyclic graph of `depth + 1` distinct objects whose expanded
/// occurrence count is `2^(depth + 1) - 1`. The counters record how many times
/// admission enumerated each level and are returned outermost first, so a test
/// can bound the work the adapter performed without ever expanding the graph
/// itself.
pub(in crate::stdlib::canonical_value) fn shared_sequence(
    depth: usize,
) -> (Value, Vec<Arc<AtomicUsize>>) {
    let mut counters = Vec::with_capacity(depth);
    let mut current = Value::from(0_i64);
    for _ in 0..depth {
        let (node, calls) = fixture(
            ObjectRepr::Seq,
            Some(2),
            Materialized::Values(vec![current.clone(), current.clone()]),
        );
        counters.push(calls);
        current = node;
    }
    counters.reverse();
    (current, counters)
}

/// Build an independently allocated tree with the same shape as
/// [`shared_sequence`].
///
/// Each recursive call constructs a fresh child, so no two occurrences share an
/// object and the value is its own expansion. Comparing its canonical key with
/// the shared graph's is what shows that admission charges occurrences while
/// the key depends only on content.
pub(in crate::stdlib::canonical_value) fn expanded_sequence(depth: usize) -> Value {
    if depth == 0 {
        Value::from(0_i64)
    } else {
        Value::from(vec![
            expanded_sequence(depth - 1),
            expanded_sequence(depth - 1),
        ])
    }
}
