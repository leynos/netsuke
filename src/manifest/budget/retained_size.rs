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
use minijinja::value::{Value, ValueKind};
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
    walk(Work::Item(root.clone()), ceiling)
}

/// Return the modelled bytes retained by one serialized JSON value.
///
/// Returns `None` when the running total would exceed `ceiling` or overflow
/// `usize`.
pub(crate) fn json_value_bytes(root: &ManifestValue, ceiling: usize) -> Option<usize> {
    walk(Work::Json(root), ceiling)
}

/// Walk one retained value graph with an explicit work stack.
///
/// Both value families share this traversal so the ceiling and arithmetic
/// policy cannot drift between them.
fn walk(root: Work<'_>, ceiling: usize) -> Option<usize> {
    let mut total = 0usize;
    let mut stack = vec![root];
    while let Some(work) = stack.pop() {
        total = match work {
            Work::Item(value) => charge_item_node(&value, total, ceiling, &mut stack)?,
            Work::Json(value) => charge_json_node(value, total, ceiling, &mut stack)?,
            Work::Items(items) => {
                push_next(items, Work::Items, &mut stack);
                total
            }
            Work::Pairs(pairs) => advance_pairs(pairs, total, ceiling, &mut stack)?,
        };
    }
    Some(total)
}

/// Charge one item node and queue its children for the same traversal.
fn charge_item_node<'a>(
    value: &Value,
    total: usize,
    ceiling: usize,
    stack: &mut Vec<Work<'a>>,
) -> Option<usize> {
    let charged = add(total, NODE_BYTES, ceiling)?;
    match value.kind() {
        ValueKind::String => add(charged, value.as_str().map_or(0, str::len), ceiling),
        ValueKind::Bytes => {
            let bytes = value.as_bytes().map_or(0, <[u8]>::len);
            add(charged, bytes.checked_mul(NODE_BYTES)?, ceiling)
        }
        ValueKind::Seq | ValueKind::Iterable => {
            let items: BoxedWork<'a> = Box::new(value.try_iter().ok()?.map(Work::Item));
            stack.push(Work::Items(items));
            Some(charged)
        }
        ValueKind::Map => {
            let pairs = value.as_object()?.try_iter_pairs()?;
            let entries: BoxedPairs<'a> = Box::new(
                pairs.map(move |(key, item)| (mini_key_bytes(&key, ceiling), Work::Item(item))),
            );
            stack.push(Work::Pairs(entries));
            Some(charged)
        }
        // Scalars, undefined, plain objects, invalid values, and any engine
        // kind this version does not model charge their node alone.
        _ => Some(charged),
    }
}

/// Charge one JSON node and queue its children for the same traversal.
fn charge_json_node<'a>(
    value: &'a ManifestValue,
    total: usize,
    ceiling: usize,
    stack: &mut Vec<Work<'a>>,
) -> Option<usize> {
    let charged = add(total, NODE_BYTES, ceiling)?;
    match value {
        ManifestValue::String(text) => add(charged, text.len(), ceiling),
        ManifestValue::Array(items) => {
            let work: BoxedWork<'a> = Box::new(items.iter().map(Work::Json));
            stack.push(Work::Items(work));
            Some(charged)
        }
        ManifestValue::Object(map) => {
            let entries: BoxedPairs<'a> = Box::new(
                map.iter()
                    .map(|(key, entry)| (map_entry_bytes(key), Work::Json(entry))),
            );
            stack.push(Work::Pairs(entries));
            Some(charged)
        }
        _ => Some(charged),
    }
}

/// Queue the next sequence item, stopping at exhaustion.
fn push_next<'a>(
    mut items: BoxedWork<'a>,
    resume: fn(BoxedWork<'a>) -> Work<'a>,
    stack: &mut Vec<Work<'a>>,
) {
    if let Some(work) = items.next() {
        stack.push(resume(items));
        stack.push(work);
    }
}

/// Charge one map key and queue its value, stopping at exhaustion.
fn advance_pairs<'a>(
    mut pairs: BoxedPairs<'a>,
    total: usize,
    ceiling: usize,
    stack: &mut Vec<Work<'a>>,
) -> Option<usize> {
    pairs.next().map_or(Some(total), |(key_bytes, work)| {
        stack.push(Work::Pairs(pairs));
        let charged = add(total, key_bytes, ceiling)?;
        stack.push(work);
        Some(charged)
    })
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

/// A sequence of pending child nodes.
type BoxedWork<'a> = Box<dyn Iterator<Item = Work<'a>> + 'a>;

/// A sequence of pending map pairs, each with its modelled key charge.
type BoxedPairs<'a> = Box<dyn Iterator<Item = (usize, Work<'a>)> + 'a>;

/// One pending node in a retained-value traversal.
enum Work<'a> {
    /// Visit one owned `MiniJinja` item value.
    Item(Value),
    /// Visit one borrowed JSON value.
    Json(&'a ManifestValue),
    /// Continue a sequence of child nodes.
    Items(BoxedWork<'a>),
    /// Continue a map of charged keys and child nodes.
    Pairs(BoxedPairs<'a>),
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
/// strings; the key's own structure is charged as well. Exceeding `ceiling`
/// saturates to `usize::MAX`, which the traversal's next charge refuses.
fn mini_key_bytes(key: &Value, ceiling: usize) -> usize {
    let base = size_of::<String>() + INDEX_ENTRY_OVERHEAD;
    key.as_str().map_or_else(
        || {
            let nested = item_value_bytes(key, ceiling.saturating_sub(base)).unwrap_or(usize::MAX);
            base.saturating_add(nested).saturating_add(NODE_BYTES)
        },
        |text| base.saturating_add(text.len()),
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
        let value = Value::make_object_iterable((), move |()| Box::new(endless()));

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
