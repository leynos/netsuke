//! Steps for bounded canonical admission of shared host graphs.
//!
//! These scenarios cover the resource contract rather than value identity: a
//! shared graph must derive the same key as its expansion, and a graph whose
//! expansion is exponential must be refused as a typed resource refusal. Both
//! Given steps therefore assert that the fixture really is shared — two items
//! that are clones of one host object — because a scenario built on an
//! ordinary tree would pass while proving nothing.

use std::sync::Arc;

use anyhow::{Context as _, Result, ensure};
use minijinja::value::{DynObject, Enumerator, Object, ObjectRepr, Value};
use netsuke::stdlib::{CanonicalResource, CanonicalValueError, canonical_eq, canonical_key};
use rstest_bdd_macros::{given, then, when};

use crate::bdd::fixtures::TestWorld;

/// Depth of the shared graph whose expansion is refused.
///
/// Its expanded occurrence count is `2^65 - 1`, far beyond any host's memory,
/// so the scenario can pass only by refusing before expansion.
const EXPONENTIAL_DEPTH: usize = 64;

/// Depth of the shared graph compared with an independently built expansion.
const SHARED_DEPTH: usize = 4;

/// A sequence object that yields two clones of one child on every enumeration.
#[derive(Debug)]
struct SharedSequence {
    /// The child both items clone.
    child: Value,
}

impl Object for SharedSequence {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(vec![self.child.clone(), self.child.clone()])
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(2)
    }
}

/// Build a shared host graph of `depth` nested two-item sequences over `0`.
fn shared_sequence(depth: usize) -> Value {
    (0..depth).fold(Value::from(0_i64), |child, _| {
        Value::from_object(SharedSequence { child })
    })
}

/// Build a tree with the same shape whose every child is allocated separately.
///
/// Each level makes two independent recursive calls rather than cloning one
/// child, so the result holds no shared object and is its own expansion.
fn expanded_sequence(depth: usize) -> Value {
    if depth == 0 {
        Value::from(0_i64)
    } else {
        Value::from(vec![
            expanded_sequence(depth - 1),
            expanded_sequence(depth - 1),
        ])
    }
}

/// Assert that the root of `graph` yields two clones of one host object.
fn ensure_shares_child(graph: &Value) -> Result<()> {
    let items: Vec<Value> = graph
        .as_object()
        .and_then(DynObject::try_iter)
        .context("the shared graph root must enumerate its items")?
        .collect();
    let (Some(first), Some(second)) = (items.first(), items.get(1)) else {
        anyhow::bail!("the shared root must yield two items, got {}", items.len());
    };
    let (Some(left), Some(right)) = (
        first.downcast_object::<SharedSequence>(),
        second.downcast_object::<SharedSequence>(),
    ) else {
        anyhow::bail!("both items must be shared-sequence objects");
    };
    ensure!(
        Arc::ptr_eq(&left, &right),
        "the fixture must yield two clones of one host object",
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a shared host sequence and an independently allocated equivalent")]
pub(crate) fn shared_and_expanded(world: &TestWorld) -> Result<()> {
    let shared = shared_sequence(SHARED_DEPTH);
    ensure_shares_child(&shared)?;
    world.canonical_map_a.set(shared);
    world.canonical_map_b.set(expanded_sequence(SHARED_DEPTH));
    Ok(())
}

#[given("a deeply shared host sequence")]
pub(crate) fn deeply_shared(world: &TestWorld) -> Result<()> {
    let graph = shared_sequence(EXPONENTIAL_DEPTH);
    ensure_shares_child(&graph)?;
    world.canonical_offending.set(graph);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("canonical admission is attempted")]
pub(crate) fn attempt_admission(world: &TestWorld) -> Result<()> {
    let value = world
        .canonical_offending
        .get()
        .context("the shared graph should have been given")?;
    let Err(refusal) = canonical_key(&value) else {
        anyhow::bail!("an exponentially shared graph must be refused, not admitted");
    };
    world.canonical_refusal_message.set(refusal.to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the shared graph has the same key as its expansion")]
pub(crate) fn shared_matches_expansion(world: &TestWorld) -> Result<()> {
    let shared = world
        .canonical_map_a
        .get()
        .context("the shared graph should have been given")?;
    let expanded = world
        .canonical_map_b
        .get()
        .context("the expansion should have been given")?;
    let shared_key = world
        .canonical_key_a
        .get()
        .context("the shared key should have been derived")?;
    let expanded_key = world
        .canonical_key_b
        .get()
        .context("the expanded key should have been derived")?;

    ensure!(
        shared_key == expanded_key,
        "a shared graph must key identically to its expansion",
    );
    ensure!(
        canonical_eq(&shared, &expanded).context("both graphs are admitted")?,
        "the equality relation must agree with the key comparison",
    );
    Ok(())
}

#[then("canonical admission is refused with a resource limit")]
pub(crate) fn refused_with_resource_limit(world: &TestWorld) -> Result<()> {
    let rendered = world
        .canonical_refusal_message
        .get()
        .context("canonical admission should have been attempted")?;
    let value = world
        .canonical_offending
        .get()
        .context("the shared graph should have been given")?;
    let Err(refusal) = canonical_key(&value) else {
        anyhow::bail!("the shared graph must still be refused");
    };

    match &refusal {
        CanonicalValueError::ResourceLimitExceeded {
            resource: CanonicalResource::Nodes,
            ..
        } => {}
        other => anyhow::bail!("expected a nodes refusal, got {other}"),
    }
    ensure!(
        refusal.kind().is_none(),
        "a resource refusal names no value kind, got {refusal}",
    );
    ensure!(
        rendered == refusal.to_string(),
        "the recorded message must be the rendered refusal",
    );
    ensure!(
        rendered.contains("nodes limit of"),
        "the message must name the exhausted ceiling, got {rendered}",
    );
    Ok(())
}
