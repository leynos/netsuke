//! Model the bytes retained by cloning one expanded manifest entry.
//!
//! The model is a structural estimate, not a heap measurement and not a
//! serialized size. Each visited node is charged as one [`ManifestValue`]
//! slot, each string adds its byte length, and each order-preserving map entry
//! adds a key header, an index slot, a control byte, and the key length. A
//! source mapping and an iterator item are therefore charged before expansion
//! clones them, using checked arithmetic and an explicit work stack so that a
//! hostile document cannot turn the estimate itself into unbounded recursion
//! or allocation.
//!
//! The estimate deliberately over-approximates ordinary JSON-shaped values.
//! It cannot see allocator overhead, an order-preserving map's growth
//! capacity, or memory held by a custom object behind a `MiniJinja` iterator.
//! A lazy iterator is consumed while it is estimated; `MiniJinja` may present
//! a fresh iterator for the later serialization, so a one-shot custom iterator
//! is outside what this model can promise.

use crate::manifest::{ManifestMap, ManifestValue};
use minijinja::value::{Value, ValueIter, ValueKind};
use std::mem::size_of;

/// Model each retained node as one JSON value slot.
const NODE_BYTES: usize = size_of::<ManifestValue>();
/// Model one order-preserving map entry's index slot and control byte.
const INDEX_ENTRY_OVERHEAD: usize = size_of::<usize>() + 1;
/// Keys that expansion consumes rather than retaining in the expanded entry.
const EXPANSION_LOCAL_KEYS: [&str; 2] = ["foreach", "when"];

/// Report whether expansion removes a key instead of retaining it.
pub(crate) fn is_expansion_local_key(key: &str) -> bool {
    EXPANSION_LOCAL_KEYS.contains(&key)
}

/// Return the modelled bytes retained by cloning one source entry.
///
/// `foreach` and `when` are skipped because the clone drops them. Returns
/// `None` when the running total would exceed `ceiling` or overflow `usize`.
pub(crate) fn source_map_bytes(map: &ManifestMap, ceiling: usize) -> Option<usize> {
    let mut total = NODE_BYTES;
    if total > ceiling {
        return None;
    }
    for (key, value) in map {
        if is_expansion_local_key(key) {
            continue;
        }
        total = add(total, map_entry_bytes(key), ceiling)?;
        total = add(total, json_value_bytes(value, ceiling - total)?, ceiling)?;
    }
    Some(total)
}

/// Return the modelled bytes retained by cloning one `MiniJinja` iterator item.
///
/// Bytes are charged as one numeric node per byte because serialization
/// expands them into a number sequence. Undefined and null share one node.
/// Unknown plain objects charge one node with no interior, and an iteration
/// error is treated like an exceeded ceiling. Returns `None` when the running
/// total would exceed `ceiling` or overflow `usize`.
pub(crate) fn item_value_bytes(root: &Value, ceiling: usize) -> Option<usize> {
    let mut total = 0usize;
    let mut stack = vec![MiniWork::Value(root.clone())];
    while let Some(work) = stack.pop() {
        match work {
            MiniWork::Value(value) => {
                total = charge_item_node(&value, total, ceiling, &mut stack)?;
            }
            MiniWork::Items(items) => advance_item_values(items, &mut stack),
            MiniWork::Pairs(mut pairs) => {
                if let Some((key, value)) = pairs.next() {
                    stack.push(MiniWork::Pairs(pairs));
                    total = add(total, mini_key_bytes(&key, ceiling - total)?, ceiling)?;
                    stack.push(MiniWork::Value(value));
                }
            }
        }
    }
    Some(total)
}

/// Charge one item node and queue its children for the same traversal.
fn charge_item_node(
    value: &Value,
    total: usize,
    ceiling: usize,
    stack: &mut Vec<MiniWork>,
) -> Option<usize> {
    let total = add(total, NODE_BYTES, ceiling)?;
    match value.kind() {
        ValueKind::String => add(total, value.as_str().map_or(0, str::len), ceiling),
        ValueKind::Bytes => {
            let bytes = value.as_bytes().map_or(0, <[u8]>::len);
            add(total, bytes.checked_mul(NODE_BYTES)?, ceiling)
        }
        ValueKind::Seq | ValueKind::Iterable => {
            stack.push(MiniWork::Items(value.try_iter().ok()?));
            Some(total)
        }
        ValueKind::Map => {
            stack.push(MiniWork::Pairs(value.as_object()?.try_iter_pairs()?));
            Some(total)
        }
        // Scalars, undefined, plain objects, invalid values, and any engine
        // kind this version does not model charge their node alone.
        _ => Some(total),
    }
}

/// Return the modelled bytes retained by one serialized JSON value.
///
/// Returns `None` when the running total would exceed `ceiling` or overflow
/// `usize`.
pub(crate) fn json_value_bytes(root: &ManifestValue, ceiling: usize) -> Option<usize> {
    let mut total = 0usize;
    let mut stack = vec![JsonWork::Value(root)];
    while let Some(work) = stack.pop() {
        match work {
            JsonWork::Value(value) => {
                total = charge_json_node(value, total, ceiling, &mut stack)?;
            }
            JsonWork::Items(items) => advance_json_items(items, &mut stack),
            JsonWork::Entries(mut entries) => {
                if let Some((key, value)) = entries.next() {
                    stack.push(JsonWork::Entries(entries));
                    total = add(total, map_entry_bytes(key), ceiling)?;
                    stack.push(JsonWork::Value(value));
                }
            }
        }
    }
    Some(total)
}

/// Charge one JSON node and queue its children for the same traversal.
fn charge_json_node<'a>(
    value: &'a ManifestValue,
    total: usize,
    ceiling: usize,
    stack: &mut Vec<JsonWork<'a>>,
) -> Option<usize> {
    let total = add(total, NODE_BYTES, ceiling)?;
    match value {
        ManifestValue::String(text) => add(total, text.len(), ceiling),
        ManifestValue::Array(items) => {
            stack.push(JsonWork::Items(items.iter()));
            Some(total)
        }
        ManifestValue::Object(map) => {
            stack.push(JsonWork::Entries(map.iter()));
            Some(total)
        }
        _ => Some(total),
    }
}

/// Queue the next value-backed sequence item, stopping at exhaustion.
fn advance_item_values(mut items: ValueIter, stack: &mut Vec<MiniWork>) {
    if let Some(value) = items.next() {
        stack.push(MiniWork::Items(items));
        stack.push(MiniWork::Value(value));
    }
}

/// Queue the next JSON array item, stopping at exhaustion.
fn advance_json_items<'a>(
    mut items: std::slice::Iter<'a, ManifestValue>,
    stack: &mut Vec<JsonWork<'a>>,
) {
    if let Some(value) = items.next() {
        stack.push(JsonWork::Items(items));
        stack.push(JsonWork::Value(value));
    }
}

/// Return the modelled overhead of the keys `foreach` injects into an entry.
///
/// Every retained expansion receives `vars.item` and `vars.index`; a
/// conditional entry also receives `conditional`. The source may already
/// supply `vars`, so all four keys are charged to stay conservative.
pub(crate) const fn iteration_overhead() -> usize {
    map_entry_bytes_const("item")
        + map_entry_bytes_const("index")
        + map_entry_bytes_const("vars")
        + map_entry_bytes_const("conditional")
        + 4 * NODE_BYTES
}

/// Work item for the non-recursive JSON traversal.
enum JsonWork<'a> {
    /// Visit one retained JSON value.
    Value(&'a ManifestValue),
    /// Continue a retained JSON array.
    Items(std::slice::Iter<'a, ManifestValue>),
    /// Continue a retained JSON object.
    Entries(serde_json::map::Iter<'a>),
}

/// Work item for the non-recursive `MiniJinja` traversal.
enum MiniWork {
    /// Visit one owned iterator value.
    Value(Value),
    /// Continue a value-backed sequence.
    Items(ValueIter),
    /// Continue a map's owned key/value pairs.
    Pairs(Box<dyn Iterator<Item = (Value, Value)> + Send + Sync>),
}

/// Add one modelled amount, refusing values beyond the ceiling.
const fn add(total: usize, amount: usize, ceiling: usize) -> Option<usize> {
    match total.checked_add(amount) {
        Some(next) if next <= ceiling => Some(next),
        _ => None,
    }
}

/// Return the modelled retained bytes of one JSON map entry key.
const fn map_entry_bytes_const(key: &str) -> usize {
    size_of::<String>() + INDEX_ENTRY_OVERHEAD + key.len()
}

/// Return the modelled retained bytes of one JSON map entry key.
const fn map_entry_bytes(key: &str) -> usize {
    map_entry_bytes_const(key)
}

/// Return the modelled retained bytes of one `MiniJinja` map key.
///
/// A non-string key also needs a string node because JSON object keys are
/// strings; the key's own structure is charged as well.
fn mini_key_bytes(key: &Value, ceiling: usize) -> Option<usize> {
    let base = size_of::<String>() + INDEX_ENTRY_OVERHEAD;
    key.as_str().map_or_else(
        || {
            let nested = item_value_bytes(key, ceiling.checked_sub(base)?)?;
            add(add(base, nested, ceiling)?, NODE_BYTES, ceiling)
        },
        |text| add(base, text.len(), ceiling),
    )
}

#[cfg(test)]
mod tests {
    //! Pin the structural estimator's model and its early-termination bounds.

    use super::*;
    use proptest::prelude::*;

    /// Build one arbitrary JSON value with bounded recursion.
    fn json_strategy() -> impl Strategy<Value = ManifestValue> {
        let leaf = prop_oneof![
            Just(ManifestValue::Null),
            any::<bool>().prop_map(ManifestValue::Bool),
            any::<i64>().prop_map(|number| ManifestValue::Number(number.into())),
            ".{0,16}".prop_map(ManifestValue::String),
        ];
        leaf.prop_recursive(4, 32, 6, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(ManifestValue::Array),
                prop::collection::vec(("[a-z]{0,8}", inner), 0..6)
                    .prop_map(|entries| ManifestValue::Object(entries.into_iter().collect())),
            ]
        })
    }

    #[test]
    fn undefined_null_bool_and_number_charge_one_node() {
        for value in [
            Value::UNDEFINED,
            Value::from(()),
            Value::from(true),
            Value::from(7),
        ] {
            assert_eq!(
                item_value_bytes(&value, NODE_BYTES),
                Some(NODE_BYTES),
                "unexpected charge for {value:?}"
            );
        }
    }

    #[test]
    fn strings_and_bytes_charge_their_payload() {
        assert_eq!(
            json_value_bytes(&ManifestValue::String("abcd".into()), NODE_BYTES + 4),
            Some(NODE_BYTES + 4),
            "a JSON string must charge its node and payload"
        );
        assert_eq!(
            item_value_bytes(&Value::from("abcd"), NODE_BYTES + 4),
            Some(NODE_BYTES + 4),
            "a MiniJinja string must charge its node and payload"
        );
        assert_eq!(
            item_value_bytes(&Value::from_bytes(vec![0; 4]), 5 * NODE_BYTES),
            Some(5 * NODE_BYTES),
            "bytes must charge one numeric node per byte"
        );
        assert_eq!(
            item_value_bytes(&Value::from_bytes(vec![0; 4]), 5 * NODE_BYTES - 1),
            None,
            "one byte below the byte charge must be refused"
        );
    }

    #[test]
    fn non_string_keys_charge_a_key_estimate_and_a_string_node() {
        let mut map = std::collections::BTreeMap::new();
        map.insert(7i64, 9i64);
        let value = Value::from_serialize(&map);
        let expected = 4 * NODE_BYTES + size_of::<String>() + INDEX_ENTRY_OVERHEAD;

        assert_eq!(
            item_value_bytes(&value, expected),
            Some(expected),
            "a numeric key must charge its estimate and a string node"
        );
        assert_eq!(
            item_value_bytes(&value, expected - 1),
            None,
            "one byte below the numeric-key charge must be refused"
        );
    }

    #[test]
    fn source_estimates_skip_keys_expansion_removes() {
        let full: ManifestMap = serde_json::from_str(
            r#"{"name":"n","command":"echo ok","foreach":["a","b"],"when":"true"}"#,
        )
        .expect("source fixture must parse");
        let retained: ManifestMap = serde_json::from_str(r#"{"name":"n","command":"echo ok"}"#)
            .expect("retained fixture must parse");

        assert_eq!(
            source_map_bytes(&full, usize::MAX),
            source_map_bytes(&retained, usize::MAX),
            "foreach and when must not contribute to the retained source estimate"
        );
    }

    #[test]
    fn traversal_stops_at_the_ceiling_for_an_endless_iterable() {
        let endless = || std::iter::repeat_with(|| Value::from(1));
        let value = Value::make_object_iterable((), move |_| Box::new(endless()));

        assert_eq!(
            item_value_bytes(&value, 1_000),
            None,
            "an endless iterable must stop at the ceiling"
        );
    }

    proptest! {
        #[test]
        fn json_round_trip_estimate_is_at_least_the_json_estimate(
            value in json_strategy(),
        ) {
            let json_bytes = json_value_bytes(&value, usize::MAX)
                .expect("a bounded JSON value must fit an unbounded ceiling");
            let mini_bytes = item_value_bytes(&Value::from_serialize(&value), usize::MAX)
                .expect("a bounded JSON value must fit an unbounded ceiling");
            prop_assert!(
                mini_bytes >= json_bytes,
                "MiniJinja estimate {mini_bytes} must cover JSON estimate {json_bytes}"
            );
        }

        #[test]
        fn nested_containers_never_shrink_the_estimate(value in json_strategy()) {
            let inner = json_value_bytes(&value, usize::MAX)
                .expect("a bounded JSON value must fit an unbounded ceiling");
            let nested = json_value_bytes(&ManifestValue::Array(vec![value]), usize::MAX)
                .expect("a bounded JSON value must fit an unbounded ceiling");
            prop_assert!(nested > inner, "nesting must add the container node");
        }
    }
}
