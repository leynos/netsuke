//! Collection filters available in the `MiniJinja` standard library.
//!
//! [`register_filters`] is the only caller that may expose these filters to a
//! template environment, and every registration path — full manifest
//! rendering, discovery, and the manifest-query surface — goes through it.
//! The `flatten` traversal and its resource allowance live in the [`flatten`]
//! child module so the bound has one owner; the remaining filters stay here.

use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

use minijinja::{
    Environment, Error, ErrorKind,
    value::{Enumerator, Object, ObjectRepr, Value, ValueKind},
};

use crate::localization::{self, keys};

mod flatten;

#[cfg(test)]
mod flatten_property;

#[cfg(test)]
mod tests;

/// Register the collection filters (`uniq`, `flatten`, `compact`, `group_by`)
/// on an environment.
///
/// One [`flatten::FlattenBudget`] is created here and captured by the
/// `flatten` filter, so every call on this environment draws from the same
/// allowance. An environment is built once per manifest load, which is the
/// scope the manifest budget uses.
pub(crate) fn register_filters(env: &mut Environment<'_>) {
    register_filters_with_limits(env, flatten::FlattenLimits::DEFAULT);
}

/// Register the collection filters under an explicit flatten allowance.
///
/// This is the single registration implementation; [`register_filters`] passes
/// the production ceilings. The extra parameter is what lets the sibling
/// `tests` module drive the *registration* path — not just the filter — with
/// ceilings small enough that a cumulative breach is reached in a few calls.
fn register_filters_with_limits(env: &mut Environment<'_>, limits: flatten::FlattenLimits) {
    let flatten_budget = Arc::new(flatten::FlattenBudget::new(limits));
    env.add_filter("uniq", |values: Value| uniq_filter(&values));
    env.add_filter("flatten", move |values: Value| {
        flatten::flatten_filter(&values, &flatten_budget)
    });
    env.add_filter("compact", |values: Value| compact_filter(&values));
    env.add_filter("group_by", |values: Value, attr: String| {
        group_by_filter(&values, &attr)
    });
}

/// Object exposing grouped filter results to templates as a map keyed by the
/// original group keys.
#[derive(Debug)]
struct GroupedValues {
    /// Groups keyed by their original `Value` keys.
    groups: IndexMap<Value, Vec<Value>>,
    /// String labels mapped to their canonical group keys for template access.
    string_keys: IndexMap<String, Value>,
}

impl GroupedValues {
    /// Construct the object, indexing string keys for attribute access while
    /// preserving the original key types.
    fn new(groups: IndexMap<Value, Vec<Value>>) -> Self {
        let mut string_keys = IndexMap::new();
        // POLONIUS-REFUSED(miss-dominant): group keys are unique, so this
        // first-wins registration almost always inserts; the owned-key
        // `entry` form pays nothing on the rare duplicate-label hit and a
        // borrow-returning accessor would buy nothing here.
        for key in groups.keys() {
            if let Some(label) = key.as_str() {
                string_keys
                    .entry(label.to_owned())
                    .or_insert_with(|| key.clone());
            }
        }
        Self {
            groups,
            string_keys,
        }
    }
}

impl Object for GroupedValues {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let keys: Vec<Value> = self.groups.keys().cloned().collect();
        Enumerator::Values(keys)
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        if let Some(name) = key.as_str()
            && let Some(actual_key) = self.string_keys.get(name)
        {
            return self
                .groups
                .get(actual_key)
                .map(|items| Value::from_serialize(items.clone()));
        }

        self.groups
            .get(key)
            .map(|items| Value::from_serialize(items.clone()))
    }
}

/// Deduplicate sequence items, preserving first-seen order.
///
/// # Errors
///
/// Returns an error when a non-sequence value cannot be iterated.
fn uniq_filter(values: &Value) -> Result<Value, Error> {
    let iter = values.try_iter()?;
    let mut uniques: IndexSet<Value> = IndexSet::new();

    for item in iter {
        uniques.insert(item);
    }

    let items: Vec<_> = uniques.into_iter().collect();
    Ok(Value::from_serialize(items))
}

/// Report whether a member is dropped by `compact`.
///
/// Only `none`, undefined, and the empty string are blank. `0`, `false`, `[]`,
/// `{}`, and a whitespace-only string are values and are retained; naming the
/// predicate keeps that asymmetry visible to the next reader.
///
/// The empty-string arm tests [`ValueKind::String`] rather than asking
/// `Value::as_str`. That method answers for well-formed UTF-8 *bytes* too, so
/// `Value::from_bytes(vec![])` would report an empty string and be dropped —
/// silently discarding a byte array on the strength of a text rule that does
/// not apply to it. Nothing in Netsuke constructs such a value today
/// ([`crate::stdlib::value_from_bytes`] normalises empty bytes to a string),
/// but the predicate now states the rule it actually means.
fn is_blank(value: &Value) -> bool {
    value.is_none()
        || value.is_undefined()
        || (value.kind() == ValueKind::String && value.as_str().is_some_and(str::is_empty))
}

/// Drop blank members from a sequence, preserving order.
///
/// # Errors
///
/// Returns an error naming the received kind when the subject is not a
/// sequence. See decision D8: `Value::try_iter()` is not a sequence check — it
/// accepts a map (yielding its keys) and a string (yielding its characters), so
/// `{{ my_map | compact }}` would quietly return the map's keys.
fn compact_filter(values: &Value) -> Result<Value, Error> {
    let kind = values.kind();
    if !matches!(kind, ValueKind::Seq | ValueKind::Iterable) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_COLLECTIONS_COMPACT_NOT_SEQUENCE)
                .with_arg("kind", kind.to_string())
                .to_string(),
        ));
    }

    let kept: Vec<Value> = values.try_iter()?.filter(|item| !is_blank(item)).collect();
    Ok(Value::from_serialize(kept))
}

/// Cluster items of `values` by the resolved value of `attr`, preserving
/// first-key-seen order.
///
/// # Errors
///
/// Returns an error when `attr` is empty or an item has no resolvable value
/// for it.
fn group_by_filter(values: &Value, attr: &str) -> Result<Value, Error> {
    if attr.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_COLLECTIONS_GROUP_BY_EMPTY_ATTR).to_string(),
        ));
    }

    let mut groups: IndexMap<Value, Vec<Value>> = IndexMap::new();
    let iter = values.try_iter()?;

    for item in iter {
        let key_value = resolve_group_key(&item, attr)?;
        groups.entry(key_value).or_default().push(item);
    }

    Ok(Value::from_object(GroupedValues::new(groups)))
}

/// Resolve the grouping key for an item, trying attribute access and
/// falling back to item access.
///
/// # Errors
///
/// Returns an error when neither attribute nor item access resolves a value
/// for the key, or when the resolved value is undefined.
fn resolve_group_key(item: &Value, attr: &str) -> Result<Value, Error> {
    match item.get_attr(attr) {
        Ok(value) => ensure_resolved(value, attr, item),
        Err(err) if err.kind() == ErrorKind::InvalidOperation => {
            let key = item.get_item(&Value::from(attr))?;
            ensure_resolved(key, attr, item)
        }
        Err(err) => Err(err),
    }
}

/// Return `value` when defined, otherwise raise an unresolved-key error for
/// `item`.
fn ensure_resolved(value: Value, attr: &str, item: &Value) -> Result<Value, Error> {
    if value.is_undefined() {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_COLLECTIONS_GROUP_BY_UNRESOLVED)
                .with_arg("attr", attr)
                .with_arg("kind", item.kind().to_string())
                .to_string(),
        ))
    } else {
        Ok(value)
    }
}
