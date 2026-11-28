//! Shared Proptest strategy for arbitrary JSON values.

use proptest::prelude::*;
use serde_json::{Map, Number, Value};

/// Generate every JSON value kind with bounded recursive containers.
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
