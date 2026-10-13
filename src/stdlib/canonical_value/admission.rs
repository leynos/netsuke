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
//! # Host precondition
//!
//! `MiniJinja` represents maps, sequences, and custom objects through an
//! `Object` trait whose enumeration the adapter cannot police. Admission
//! therefore assumes the caller's values are finite, stable, and acyclic: a
//! single traversal reads each member once and never re-reads the host object,
//! so a value that mutates between calls, enumerates without end, or contains
//! itself is outside the enforceable contract. `MiniJinja` exposes no public
//! identity hook with which this adapter could detect a cycle, so none is
//! claimed.
//!
//! # Traversal order
//!
//! A mapping's members are collected once, then scanned for a non-string key
//! before any value is visited, and the accepted keys are sorted before their
//! values are snapshotted. Both the canonical bytes and the first reported
//! refusal are therefore independent of the order the host enumerates.

use std::collections::BTreeMap;

use minijinja::value::{Value, ValueKind};

use super::error::{CanonicalValueError, SnapshotPath};
use super::node::SnapshotNode;

/// Traverse `value` once and return the snapshot the canonicalizer serializes.
pub(super) fn snapshot(value: &Value) -> Result<SnapshotNode, CanonicalValueError> {
    admit(value, &SnapshotPath::root())
}

/// Admit one value, reporting any refusal at `path`.
fn admit(value: &Value, path: &SnapshotPath) -> Result<SnapshotNode, CanonicalValueError> {
    match value.kind() {
        ValueKind::None => Ok(SnapshotNode::Null),
        ValueKind::Bool => Ok(SnapshotNode::Bool(value.is_true())),
        ValueKind::String => admit_string(value, path),
        ValueKind::Number => admit_number(value, path),
        ValueKind::Seq => admit_sequence(value, path),
        ValueKind::Map => admit_mapping(value, path),
        unsupported => Err(CanonicalValueError::unsupported(unsupported, path)),
    }
}

/// Admit a string value.
///
/// Both internal string representations report [`ValueKind::String`], so this
/// reads the text rather than branching on how it is stored. The read is
/// refuted by the kind dispatch above and exists so the function stays total.
fn admit_string(value: &Value, path: &SnapshotPath) -> Result<SnapshotNode, CanonicalValueError> {
    value
        .as_str()
        .map(|text| SnapshotNode::String(text.to_owned()))
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))
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
fn admit_sequence(value: &Value, path: &SnapshotPath) -> Result<SnapshotNode, CanonicalValueError> {
    let items = collect_items(value, path)?;
    let mut nodes = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        nodes.push(admit(item, &path.with_item(index))?);
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
fn admit_mapping(value: &Value, path: &SnapshotPath) -> Result<SnapshotNode, CanonicalValueError> {
    let members = collect_members(value, path)?;
    refute_non_string_key(&members, path)?;
    let mut nodes = BTreeMap::new();
    for (key, item) in sort_members(members, path)? {
        // Build the member path first, so the key can be moved into the map
        // rather than cloned to outlive the borrow.
        let node = admit(&item, &path.with_member(&key))?;
        nodes.insert(key, node);
    }
    Ok(SnapshotNode::Map(nodes))
}

/// Collect the members of a mapping representation exactly once.
fn collect_members(
    value: &Value,
    path: &SnapshotPath,
) -> Result<Vec<(Value, Value)>, CanonicalValueError> {
    let object = value
        .as_object()
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    let members = object
        .try_iter_pairs()
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    Ok(members.collect())
}

/// Collect the items of a sequence representation exactly once.
///
/// The host enumerator is consulted once and its self-reported length is
/// checked against what it actually yielded. A length that disagrees means the
/// host object is not stable, which the caller's precondition excludes.
fn collect_items(value: &Value, path: &SnapshotPath) -> Result<Vec<Value>, CanonicalValueError> {
    let object = value
        .as_object()
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    let items = object
        .try_iter()
        .ok_or_else(|| CanonicalValueError::invalid_traversal(path))?;
    let collected: Vec<Value> = items.collect();
    if has_stable_length(value, collected.len()) {
        Ok(collected)
    } else {
        Err(CanonicalValueError::invalid_traversal(path))
    }
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
fn sort_members(
    members: Vec<(Value, Value)>,
    path: &SnapshotPath,
) -> Result<Vec<(String, Value)>, CanonicalValueError> {
    let mut sorted = BTreeMap::new();
    for (key, item) in members {
        let Some(name) = key.as_str().map(str::to_owned) else {
            return Err(CanonicalValueError::non_string_key(key.kind(), path));
        };
        if sorted.insert(name, item).is_some() {
            return Err(CanonicalValueError::invalid_traversal(path));
        }
    }
    Ok(sorted.into_iter().collect())
}
