//! Steps for canonical value identity in behavioural tests.
//!
//! The scenarios drive the same three functions the unit tests do, but through
//! the public surface alone: no step reaches into the snapshot tree, and the
//! assertions read only the observables the earlier steps stored. Every refusal
//! case therefore shows the typed error a caller actually receives.

use std::sync::Arc;

use anyhow::{Context as _, Result, ensure};
use minijinja::value::{Enumerator, Object, ObjectRepr, Value, ValueKind};
use netsuke::stdlib::{canonical_eq, canonical_key, stable_deduplicate};
use rstest_bdd_macros::{given, then, when};

use crate::bdd::fixtures::TestWorld;

/// A string-keyed map that enumerates in the order it was built with.
///
/// `MiniJinja`'s own map representation iterates in sorted order and this build
/// has no insertion-preserving one, so expressing two orders of a single member
/// set needs a host object. The order is the whole point of the fixture.
#[derive(Debug)]
struct OrderedMap {
    /// Members in the order the fixture was built with.
    members: Vec<(&'static str, i64)>,
}

impl Object for OrderedMap {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(
            self.members
                .iter()
                .map(|(name, _)| Value::from(*name))
                .collect(),
        )
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let name = key.as_str()?;
        self.members
            .iter()
            .find(|(member, _)| *member == name)
            .map(|(_, value)| Value::from(*value))
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(self.members.len())
    }
}

/// Build a map value whose members enumerate in the order given.
fn ordered_map(members: &[(&'static str, i64)]) -> Value {
    Value::from_object(OrderedMap {
        members: members.to_vec(),
    })
}

/// Read back the order in which a host map enumerates its member names.
fn host_order(value: &Value) -> Option<Vec<String>> {
    let iter = value.as_object()?.try_iter()?;
    Some(
        iter.filter_map(|key| key.as_str().map(str::to_owned))
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("two string-keyed maps with identical members in different orders")]
pub(crate) fn two_reordered_maps(world: &TestWorld) -> Result<()> {
    let first = ordered_map(&[("alpha", 1), ("beta", 2), ("gamma", 3)]);
    let second = ordered_map(&[("gamma", 3), ("beta", 2), ("alpha", 1)]);
    let differ = host_order(&first) != host_order(&second);

    world.canonical_host_orders_differ.set(differ);
    world.canonical_map_a.set(first);
    world.canonical_map_b.set(second);
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("a materialized sequence with repeated canonical values")]
pub(crate) fn repeated_canonical_values(world: &TestWorld) -> Result<()> {
    let first = ordered_map(&[("alpha", 1), ("beta", 2)]);
    let reversed = ordered_map(&[("beta", 2), ("alpha", 1)]);
    let sequence = vec![
        first,
        Value::from("zeta"),
        reversed,
        Value::from("alpha"),
        Value::from("zeta"),
    ];
    world.canonical_sequence.set(sequence);
    Ok(())
}

#[given("a value outside the canonical JSON domain")]
pub(crate) fn out_of_domain_value(world: &TestWorld) -> Result<()> {
    let value = Value::from_bytes(vec![1, 2, 3]);
    ensure!(
        canonical_key(&value).is_err(),
        "the fixture must be outside the domain or the scenario proves nothing",
    );
    world.canonical_offending.set(value);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("their canonical keys are derived")]
pub(crate) fn derive_canonical_keys(world: &TestWorld) -> Result<()> {
    let first = world
        .canonical_map_a
        .get()
        .context("the first map should have been given")?;
    let second = world
        .canonical_map_b
        .get()
        .context("the second map should have been given")?;

    world.canonical_key_a.set(canonical_key(&first)?);
    world.canonical_key_b.set(canonical_key(&second)?);
    Ok(())
}

#[when("stable deduplication is applied")]
pub(crate) fn apply_deduplication(world: &TestWorld) -> Result<()> {
    let sequence = world
        .canonical_sequence
        .get()
        .context("the sequence should have been given")?;
    let retained =
        stable_deduplicate(&sequence).context("every value in the fixture sequence is admitted")?;

    let indices: Vec<usize> = retained
        .iter()
        .filter_map(|value| sequence.iter().position(|item| std::ptr::eq(item, *value)))
        .collect();
    let order: Vec<String> = retained
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    world.canonical_retained_indices.set(indices);
    world.canonical_retained_order.set(order);
    Ok(())
}

#[when("its canonical key is requested")]
pub(crate) fn request_canonical_key(world: &TestWorld) -> Result<()> {
    let value = world
        .canonical_offending
        .get()
        .context("the offending value should have been given")?;
    let Err(refusal) = canonical_key(&value) else {
        anyhow::bail!("the offending value must be refused, not admitted");
    };

    let Some(kind) = refusal.kind() else {
        anyhow::bail!("the refusal must name the offending kind: {refusal}");
    };
    world.canonical_refusal_kind.set(kind.to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the keys are equal")]
pub(crate) fn keys_are_equal(world: &TestWorld) -> Result<()> {
    let first = world
        .canonical_key_a
        .get()
        .context("the first key should have been derived")?;
    let second = world
        .canonical_key_b
        .get()
        .context("the second key should have been derived")?;

    ensure!(
        world.canonical_host_orders_differ.get() == Some(true),
        "the fixtures must enumerate differently or they discriminate nothing",
    );
    ensure!(first == second, "reordered maps must derive equal keys");

    let left = world
        .canonical_map_a
        .get()
        .context("the first map should have been given")?;
    let right = world
        .canonical_map_b
        .get()
        .context("the second map should have been given")?;
    ensure!(
        canonical_eq(&left, &right).context("both maps are admitted")?,
        "the equality relation must agree with the key comparison",
    );
    Ok(())
}

#[then("the original first values appear once in first-appearance order")]
pub(crate) fn first_values_in_order(world: &TestWorld) -> Result<()> {
    let indices = world
        .canonical_retained_indices
        .get()
        .context("deduplication should have run")?;
    let order = world
        .canonical_retained_order
        .get()
        .context("deduplication should have run")?;

    // The fixture repeats the map at index 2 and `zeta` at index 4, so only
    // the first of each survives; the survivors are in input order.
    ensure!(
        indices == vec![0, 1, 3],
        "the first of each canonical value must be retained in input order, got {indices:?}",
    );
    ensure!(
        order.len() == indices.len(),
        "every retained index must have a rendered value",
    );

    // The retained map at index 0 renders first, so the surviving strings are
    // the rendered slice's tail: `zeta` from index 1, then `alpha` from index
    // 3. Comparing the whole tail, rather than one element of it, is what makes
    // the order claim mean something.
    let surviving_strings: Vec<&str> = order.iter().skip(1).map(String::as_str).collect();
    ensure!(
        surviving_strings == ["zeta", "alpha"],
        "the surviving string values must keep their first-appearance order, got {order:?}",
    );
    Ok(())
}

#[then("a typed error identifies its value kind")]
pub(crate) fn typed_error_names_kind(world: &TestWorld) -> Result<()> {
    let kind = world
        .canonical_refusal_kind
        .get()
        .context("the refusal should have been recorded")?;

    ensure!(
        kind == ValueKind::Bytes.to_string(),
        "the refusal must name the bytes kind, got {kind}",
    );

    let value = world
        .canonical_offending
        .get()
        .context("the offending value should have been given")?;
    let Err(refusal) = canonical_key(&value) else {
        anyhow::bail!("the offending value must still be refused");
    };
    ensure!(
        refusal.kind() == Some(ValueKind::Bytes),
        "the typed error must report the kind structurally as well as in prose",
    );
    ensure!(
        !refusal.to_string().is_empty(),
        "the refusal must render a message for the caller",
    );
    Ok(())
}
