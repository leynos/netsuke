//! Shared fixtures and assertions for bounded canonical admission.
//!
//! Two batteries need the same support: `budget` checks the exact ceilings,
//! precedence, and operation scope, while `diagnostics` pins how each refusal
//! renders. Both need a budget built from small explicit ceilings, an assertion
//! that a refusal names a particular resource, and the shared host graph whose
//! expansion is exponential in its depth. They live here so neither battery can
//! drift onto a private copy, and the parent module re-exports them so the
//! fixtures still read as part of the shared set.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use anyhow::{Result, ensure};
use minijinja::value::{ObjectRepr, Value};

use super::*;

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
