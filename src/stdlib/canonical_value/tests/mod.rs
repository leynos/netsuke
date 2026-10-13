//! Shared fixtures and the handwritten cases for canonical value identity.
//!
//! Every case in this subtree drives the public surface — `canonical_key`,
//! `canonical_eq`, and `stable_deduplicate` — so a refusal reaches the real
//! adapter and an admission reaches the real RFC 8785 canonicalizer. Nothing
//! here tests the snapshot logic in isolation.
//!
//! The batteries are split by the kind of judgement each makes, not by line
//! count. This module owns the host fixtures and the helpers every other
//! battery builds on; `boundaries` owns the refusals; `parity` owns the checks
//! that compare emitted bytes against the canonicalizer itself; `properties`
//! owns the cases whose inputs come from a generator and the shared runners;
//! `oracle` owns the independent quadratic first-index oracle and the cases
//! that check `stable_deduplicate` against it; and `examples` owns the cases
//! whose inputs are written out by hand. A fixture lives here rather than
//! beside its first caller so that a battery cannot quietly drift onto a
//! private copy of it.
//!
//! EP-V3's independent quadratic first-index oracle lives in `oracle`, and the
//! standalone Verus proof over the abstract kernel lives in
//! `verus/canonical_value/main.rs`: the oracle is the runtime correspondence
//! test, checking retained values' identity and order against an expectation
//! computed without calling `stable_deduplicate`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context as _, Result, ensure};
use minijinja::value::{Enumerator, Object, ObjectRepr, Value, ValueKind};

use super::*;

mod boundaries;
mod examples;
mod oracle;
mod parity;
mod properties;

/// The material classes the generator must be seen to emit.
const REQUIRED_CLASSES: [&str; 7] = ["null", "bool", "int", "float", "string", "seq", "map"];

/// A string-keyed map that enumerates in the order it was built with.
///
/// `MiniJinja`'s own map representations iterate in sorted order, so they
/// cannot express two orders of one member set. That is exactly what the
/// order-independence cases need, and building the host here means the
/// enumeration order can be read back and asserted, so a fixture that failed
/// to discriminate would fail its own control.
#[derive(Debug)]
pub(super) struct OrderedMap {
    /// Members in the order the fixture was built with.
    members: Vec<(String, Value)>,
}

impl Object for OrderedMap {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let keys = self
            .members
            .iter()
            .map(|(name, _)| Value::from(name.as_str()))
            .collect();
        Enumerator::Values(keys)
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let name = key.as_str()?;
        self.members
            .iter()
            .find(|(member, _)| member == name)
            .map(|(_, value)| value.clone())
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(self.members.len())
    }
}

/// Build a string-keyed map value whose members enumerate in the order given.
///
/// The order is preserved exactly, because expressing two orders of one member
/// set is the only thing this fixture exists for. A constructor that sorted
/// would make the order-discriminating control below pass vacuously.
pub(super) fn ordered_map(members: &[(&str, i64)]) -> Value {
    Value::from_object(OrderedMap {
        members: members
            .iter()
            .map(|(name, value)| ((*name).to_owned(), Value::from(*value)))
            .collect(),
    })
}

/// Build a string-keyed map value from already-built members, sorted by name.
///
/// Sorting makes the host order a function of the content, which is what a
/// rebuilt or generated map wants: only the key is under test there, and a
/// defined host order keeps two equal maps indistinguishable at that layer.
pub(super) fn map_of(mut members: Vec<(String, Value)>) -> Value {
    members.sort_by(|(left, _), (right, _)| left.cmp(right));
    Value::from_object(OrderedMap { members })
}

/// Read back the order in which a host map enumerates its keys.
pub(super) fn enumeration_order(value: &Value) -> Option<Vec<String>> {
    let iter = value.as_object()?.try_iter()?;
    Some(
        iter.filter_map(|key| key.as_str().map(str::to_owned))
            .collect(),
    )
}

/// What a [`FixtureObject`] enumerates.
#[derive(Debug)]
pub(super) enum Materialized {
    /// Yield these sequence items, again on every enumeration.
    Values(Vec<Value>),
    /// Yield `count` freshly built items on each enumeration, so two
    /// enumerations of one host object are distinguishable.
    Renewed {
        /// How many items each enumeration yields.
        count: usize,
    },
    /// Yield these key/value pairs, again on every enumeration.
    Pairs(Vec<(Value, Value)>),
    /// Report the object as non-enumerable.
    NonEnumerable,
    /// Enumerate a fixed length through `Enumerator::Seq`, reading items by
    /// index, so a reported length can be made to disagree with the yield.
    Seeded,
}

/// A host object whose representation, length, and enumeration are all chosen
/// by the test, so every untraversable and non-string-key boundary is
/// reachable. `MiniJinja` exposes no public constructor for a map with
/// non-string keys, so this fixture is the only way to reach those refusals.
#[derive(Debug)]
pub(super) struct FixtureObject {
    /// Representation the object reports.
    repr: ObjectRepr,
    /// Length the object reports, independently of what it yields.
    len: Option<usize>,
    /// What enumeration yields.
    materialized: Materialized,
    /// Value returned for any key lookup by a `Seq`-represented object.
    member: Value,
    /// Number of times the object has been enumerated.
    calls: Arc<AtomicUsize>,
}

impl Object for FixtureObject {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        self.repr
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.materialized {
            Materialized::Values(items) => Enumerator::Values(items.clone()),
            Materialized::Renewed { count } => {
                Enumerator::Iter(Box::new((0..*count).map(|index| Value::from(index + 1))))
            }
            Materialized::Pairs(pairs) => {
                Enumerator::KeyValueIter(Box::new(pairs.clone().into_iter()))
            }
            Materialized::NonEnumerable => Enumerator::NonEnumerable,
            Materialized::Seeded => Enumerator::Seq(3),
        }
    }

    fn get_value(self: &Arc<Self>, _key: &Value) -> Option<Value> {
        Some(self.member.clone())
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        self.len
    }
}

/// Build a fixture object and the counter that records its enumerations.
pub(super) fn fixture(
    repr: ObjectRepr,
    len: Option<usize>,
    materialized: Materialized,
) -> (Value, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let object = FixtureObject {
        repr,
        len,
        materialized,
        member: Value::from(7),
        calls: Arc::clone(&calls),
    };
    (Value::from_object(object), calls)
}

/// A type whose `Serialize` always fails, to reach `ValueKind::Invalid`.
#[derive(Debug)]
pub(super) struct AlwaysFails;

impl serde::Serialize for AlwaysFails {
    fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("always fails"))
    }
}

/// A keyless iterable object, the shape that reports [`ValueKind::Iterable`].
#[derive(Debug)]
pub(super) struct TestIterable;

impl Object for TestIterable {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Iterable
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(vec![Value::from(1)])
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(1)
    }
}

/// A map holding one admitted member plus one nested object, so a refusal can
/// be reached below the root rather than only at it.
#[derive(Debug)]
pub(super) struct NestedMap {
    /// The object placed at the nested position.
    pub(super) inner: Value,
    /// An admitted member, so the outer map is otherwise valid.
    outer: Value,
}

impl Object for NestedMap {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(vec![Value::from("nested"), Value::from("ok")])
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        match key.as_str()? {
            "nested" => Some(self.inner.clone()),
            "ok" => Some(self.outer.clone()),
            _ => None,
        }
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(2)
    }
}

/// Build a map with `inner` at the `nested` member and an integer at `ok`.
pub(super) fn nested_map(inner: Value) -> Value {
    Value::from_object(NestedMap {
        inner,
        outer: Value::from(1),
    })
}

/// Return the `now()` object, or `None` when the environment refuses it.
pub(super) fn now_object() -> Option<Value> {
    let mut env = minijinja::Environment::new();
    crate::stdlib::register(&mut env).ok()?;
    let compiled = env.compile_expression("now()").ok()?;
    compiled.eval(minijinja::context! {}).ok()
}

/// Return the canonical bytes of `value`, failing on any refusal.
pub(super) fn key_bytes(value: &Value) -> Result<Vec<u8>> {
    Ok(canonical_key(value)
        .context("the value should be admitted")?
        .as_bytes()
        .to_vec())
}

/// Assert that `value` is refused with `kind` named by the typed error.
pub(super) fn ensure_refused(value: &Value, kind: ValueKind, label: &str) -> Result<()> {
    let refusal = canonical_key(value).expect_err("the value should be refused");
    ensure!(
        refusal.kind() == Some(kind),
        "{label} should report {kind}, got {:?} via {refusal}",
        refusal.kind(),
    );
    Ok(())
}

/// Every signed extreme RFC 8785 documents a rendering for.
pub(super) fn signed_extremes() -> [i128; 4] {
    [
        i128::from(i64::MIN),
        i128::from(i64::MAX),
        i128::MIN,
        i128::MAX,
    ]
}

/// Every unsigned extreme RFC 8785 documents a rendering for.
pub(super) fn unsigned_extremes() -> [u128; 2] {
    [u128::from(u64::MAX), u128::MAX]
}
