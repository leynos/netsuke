//! Shared Proptest strategy for arbitrary JSON values.
//!
//! Several suites take "some JSON value" as an input: the shell-selection
//! decoder and the manifest-expansion properties both do. They share one
//! generator rather than copying it, because a second copy is a second
//! statement of what a well-formed value is, and the two drift silently while
//! both keep passing.
//!
//! Scope and re-use policy: this module is test-only support
//! (`#[cfg(test)]`), owned by the crate root, and carries no manifest, shell,
//! or configuration knowledge. The string leaf is a parameter so a caller can
//! supply names, near misses, or template-looking text without changing the
//! generator. A caller that needs an extra leaf kind composes it at its own
//! call site; new leaf kinds do not belong here.

use proptest::prelude::*;
use serde_json::{Map, Number, Value};

/// Generate every JSON value kind with bounded recursive containers.
///
/// Leaves cover all six kinds; containers add empty and nested arrays and
/// objects, with recursion depth and width bounded so shrinking stays cheap.
/// The float arm draws only from a finite range, so it never asks
/// `Number::from_f64` for a value JSON cannot hold.
pub(crate) fn json_values(strings: BoxedStrategy<String>) -> impl Strategy<Value = Value> {
    let leaves = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|number| Value::Number(Number::from(number))),
        (-10_000.0f64..10_000.0f64).prop_filter_map("finite JSON number", |number| {
            Number::from_f64(number).map(Value::Number)
        }),
        strings.prop_map(Value::String),
    ];
    let nested = leaves.clone().prop_recursive(2, 64, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..=4).prop_map(Value::Array),
            prop::collection::btree_map("[a-z]{0,4}", inner, 0..=4)
                .prop_map(|entries| { Value::Object(Map::from_iter(entries)) }),
        ]
    });
    let containers = prop_oneof![
        Just(Value::Array(Vec::new())),
        Just(Value::Object(Map::new())),
    ];
    prop_oneof![5 => leaves, 2 => containers, 3 => nested]
}

/// Describe one of JSON's six value kinds for deterministic coverage checks.
pub(crate) fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
