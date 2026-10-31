//! Admission of `MiniJinja` host values into the canonical snapshot tree.
//!
//! This module is the boundary where a host value is either admitted or
//! refused, and it is the only place in the module that knows about
//! `MiniJinja` representations. Admission happens before any coercion: a kind
//! outside the canonical-JSON domain, a mapping key that is not a string, a
//! non-finite float, and a map or sequence that cannot be traversed are each
//! refused here, with a typed error, instead of being handed to a serializer
//! that would quietly convert them.
//!
//! # Host precondition and resource bounds
//!
//! `MiniJinja` represents maps, sequences, and custom objects through an
//! `Object` trait whose enumeration the adapter cannot police, and cloning a
//! value shares its host object, so one value may be a directed acyclic graph
//! rather than a tree. Admission therefore assumes the caller's values are
//! finite, stable, and acyclic, and it charges every *occurrence* it admits
//! against a per-operation budget: the root, each enumerated item, each member
//! pair, and the UTF-8 length of each string and member name. A shared graph is
//! expanded occurrence by occurrence, so the budget — not the host — decides
//! when that expansion stops.
//!
//! The budget bounds the work; it does not diagnose the host. A value that
//! enumerates without end, or one that contains itself, exhausts a ceiling and
//! ends as a [`CanonicalValueError::ResourceLimitExceeded`] rather than a cycle
//! report, because `MiniJinja` exposes no public identity hook with which this
//! adapter could detect a cycle. None is claimed.
//!
//! # Traversal order
//!
//! A mapping's members are collected once, then scanned for a non-string key
//! before any value is visited, and the accepted keys are sorted before their
//! values are snapshotted. Both the canonical bytes and the first reported
//! refusal are therefore independent of the order the host enumerates.

use std::collections::BTreeMap;

use minijinja::value::{DynObject, Value, ValueKind};

use super::budget::AdmissionBudget;
use super::error::{CanonicalValueError, SnapshotPath};
use super::node::SnapshotNode;

/// Traverse `value` once and return the snapshot the canonicalizer serializes.
///
/// The root occurrence is charged before the traversal starts, so a budget that
/// is already exhausted refuses even a scalar.
pub(super) fn snapshot(
    value: &Value,
    budget: &mut AdmissionBudget,
) -> Result<SnapshotNode, CanonicalValueError> {
    let path = SnapshotPath::root();
    budget.charge_node(&path)?;
    admit(value, &path, 0, budget)
}

/// Admit one value at `depth` edges below the root, reporting refusals at `path`.
///
/// The depth ceiling is checked before the value is dispatched, so a container
/// past the ceiling is refused without enumerating it. That ordering is what
/// keeps a self-referential host finite: the recursion ends at the ceiling even
/// though the adapter cannot see the cycle.
fn admit(
    value: &Value,
    path: &SnapshotPath,
    depth: usize,
    budget: &mut AdmissionBudget,
) -> Result<SnapshotNode, CanonicalValueError> {
    budget.check_depth(depth, path)?;
    match value.kind() {
        ValueKind::None => Ok(SnapshotNode::Null),
        ValueKind::Bool => Ok(SnapshotNode::Bool(value.is_true())),
        ValueKind::String => admit_string(value, path, budget),
        ValueKind::Number => admit_number(value, path),
        ValueKind::Seq => admit_sequence(value, path, depth, budget),
        ValueKind::Map => admit_mapping(value, path, depth, budget),
        unsupported => Err(CanonicalValueError::unsupported(unsupported, path)),
    }
}

/// Admit a string value, charging its UTF-8 length before it is copied.
///
/// Both internal string representations report [`ValueKind::String`], so this
/// reads the text rather than branching on how it is stored. The read is
/// refuted by the kind dispatch above and exists so the function stays total.
/// Charging before `to_owned` is what keeps the ceiling a bound on memory: a
/// refused string is never copied.
fn admit_string(
    value: &Value,
    path: &SnapshotPath,
    budget: &mut AdmissionBudget,
) -> Result<SnapshotNode, CanonicalValueError> {
    let text = value
        .as_str()
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    budget.charge_bytes(text.len(), path)?;
    Ok(SnapshotNode::String(text.to_owned()))
}

/// Admit a number, choosing between the integer and binary64 arms.
///
/// The arms must be separated by `is_integer` before either is read: the
/// primitive conversions carry a boolean arm, so an ungated integer read makes
/// `true` indistinguishable from `1`. Every integer width and the binary64
/// case share [`ValueKind::Number`], so the kind alone does not select an arm.
fn admit_number(value: &Value, path: &SnapshotPath) -> Result<SnapshotNode, CanonicalValueError> {
    if value.is_integer() {
        return admit_integer(value, path);
    }
    let number =
        f64::try_from(value.clone()).map_err(|_| CanonicalValueError::invalid_traversal(path))?;
    if number.is_finite() {
        Ok(SnapshotNode::F64(number))
    } else {
        Err(CanonicalValueError::non_finite(path))
    }
}

/// Admit an integer, keeping whether the source value was signed.
///
/// No integer width is narrowed here. The canonicalizer renders every width as
/// a binary64, so this preserves fidelity for a future serializer rather than
/// making the key injective over the integers; it does not.
fn admit_integer(value: &Value, path: &SnapshotPath) -> Result<SnapshotNode, CanonicalValueError> {
    if let Ok(signed) = i128::try_from(value.clone()) {
        return Ok(SnapshotNode::I128(signed));
    }
    u128::try_from(value.clone())
        .map(SnapshotNode::U128)
        .map_err(|_| CanonicalValueError::unsupported(ValueKind::Number, path))
}

/// Admit a sequence representation into an ordered list of snapshots.
fn admit_sequence(
    value: &Value,
    path: &SnapshotPath,
    depth: usize,
    budget: &mut AdmissionBudget,
) -> Result<SnapshotNode, CanonicalValueError> {
    let items = collect_items(value, path, budget)?;
    let mut nodes = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        nodes.push(admit(
            item,
            &path.with_item(index),
            depth.saturating_add(1),
            budget,
        )?);
    }
    Ok(SnapshotNode::Seq(nodes))
}

/// Admit a mapping representation into a key-sorted snapshot of snapshots.
///
/// The outer [`refute_non_string_key`] is not redundant with the guard inside
/// [`sort_members`], even though it rejects every key that guard would. It
/// reports the *lowest* non-string kind rather than the first one encountered,
/// so the refusal is independent of host enumeration order; the inner guard
/// exists so `sort_members` refuses a non-string key on its own terms.
fn admit_mapping(
    value: &Value,
    path: &SnapshotPath,
    depth: usize,
    budget: &mut AdmissionBudget,
) -> Result<SnapshotNode, CanonicalValueError> {
    let members = collect_members(value, path, budget)?;
    refute_non_string_key(&members, path)?;
    let mut nodes = BTreeMap::new();
    for (key, item) in sort_members(members, path, budget)? {
        // Build the member path first, so the key can be moved into the map
        // rather than cloned to outlive the borrow.
        let node = admit(
            &item,
            &path.with_member(&key),
            depth.saturating_add(1),
            budget,
        )?;
        nodes.insert(key, node);
    }
    Ok(SnapshotNode::Map(nodes))
}

/// Collect the members of a mapping representation exactly once.
///
/// A mapping needs the stability check a sequence gets, for the same reason: a
/// canonical key derived from one reading of a host that would enumerate
/// differently the second time is not a key for its value. The check itself is
/// shared through [`collect_stably`], so the two cannot drift apart.
fn collect_members(
    value: &Value,
    path: &SnapshotPath,
    budget: &mut AdmissionBudget,
) -> Result<Vec<(Value, Value)>, CanonicalValueError> {
    collect_stably(value, path, budget, DynObject::try_iter_pairs)
}

/// Collect the items of a sequence representation exactly once.
///
/// A sequence's items are its values; [`DynObject::try_iter`] is the only part
/// of this shape that differs from a mapping's, and the traversal and stability
/// rule are shared with it through [`collect_stably`].
fn collect_items(
    value: &Value,
    path: &SnapshotPath,
    budget: &mut AdmissionBudget,
) -> Result<Vec<Value>, CanonicalValueError> {
    collect_stably(value, path, budget, DynObject::try_iter)
}

/// Collect one representation's enumeration exactly once, checking its length.
///
/// This is the single traversal-and-check path, so the stability rule cannot
/// drift between the two representations that need it: `enumerate` selects the
/// representation's own iterator and the length comparison happens here for
/// both. A canonical key derived from one reading of a host that would
/// enumerate differently the second time is not a key for its value, and that
/// argument does not mention sequences or mappings at all.
///
/// The budget is spent *while* enumerating rather than after. At most one item
/// beyond the remaining allowance is pulled, and each yielded item is charged
/// before it is stored, so an unbounded or over-long enumeration ends as a
/// resource refusal before the snapshot vector is allocated and before the
/// reported length is compared. A host that reports a length disagreeing with
/// its enumeration is therefore only diagnosed when the enumeration itself fits
/// the budget.
fn collect_stably<T>(
    value: &Value,
    path: &SnapshotPath,
    budget: &mut AdmissionBudget,
    enumerate: fn(&DynObject) -> Option<Box<dyn Iterator<Item = T> + Send + Sync>>,
) -> Result<Vec<T>, CanonicalValueError> {
    let object = value
        .as_object()
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    let enumerated =
        enumerate(object).ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    let mut collected = Vec::new();
    for item in enumerated.take(budget.remaining_nodes().saturating_add(1)) {
        budget.charge_node(path)?;
        collected.push(item);
    }
    has_stable_length(value, collected.len())
        .then_some(collected)
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))
}

/// Report whether the host's self-reported length matches what it enumerated.
fn has_stable_length(value: &Value, collected: usize) -> bool {
    value.len().is_none_or(|reported| reported == collected)
}

/// Refuse a mapping whose members include a key that is not a string.
///
/// The canonicalizer does not refuse these itself: `serde_json` coerces an
/// integer key to a quoted string, so delegating the check would let `1` and
/// `"1"` collide. When several keys are non-strings the lowest kind decides,
/// which keeps the reported kind independent of host enumeration order.
fn refute_non_string_key(
    members: &[(Value, Value)],
    path: &SnapshotPath,
) -> Result<(), CanonicalValueError> {
    members
        .iter()
        .map(|(key, _)| key.kind())
        .filter(|kind| *kind != ValueKind::String)
        .min()
        .map_or(Ok(()), |kind| {
            Err(CanonicalValueError::non_string_key(kind, path))
        })
}

/// Return the members as sorted key/value pairs, refusing a repeated name.
///
/// Sorting before the values are visited makes the first reported refusal
/// independent of host enumeration order, matching the canonicalizer's
/// content-defined output. A repeated member name has no canonical form: RFC
/// 8785 canonicalizes a JSON object, and a JSON object has unique names.
///
/// A member name is charged against the byte ceiling after the non-string-key
/// guard and before it is copied into the sorting map, so the ceiling bounds
/// the map's keys as well as the admitted values. A refused name is never
/// copied, and a refused budget therefore leaves no partially built map behind.
fn sort_members(
    members: Vec<(Value, Value)>,
    path: &SnapshotPath,
    budget: &mut AdmissionBudget,
) -> Result<Vec<(String, Value)>, CanonicalValueError> {
    let mut sorted = BTreeMap::new();
    for (key, item) in members {
        let Some(name) = key.as_str() else {
            return Err(CanonicalValueError::non_string_key(key.kind(), path));
        };
        budget.charge_bytes(name.len(), path)?;
        if sorted.insert(name.to_owned(), item).is_some() {
            return Err(CanonicalValueError::invalid_traversal(path));
        }
    }
    Ok(sorted.into_iter().collect())
}
