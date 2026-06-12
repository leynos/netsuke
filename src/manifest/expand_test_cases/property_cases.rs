//! Property-based tests for manifest `foreach`/`when` expansion invariants.
//!
//! These complement the example-based cases in `condition_cases.rs` by
//! checking the expansion contract across generated inputs: entry counts,
//! index sequencing, `when` determinism, whitespace-only `when` rejection,
//! and `foreach` key removal.

use super::*;
use minijinja::Environment;
use proptest::prelude::*;
use serde_json::json;

/// Build a single-target manifest document iterating over `items`.
fn foreach_doc(items: &[String], when: Option<&str>) -> ManifestValue {
    let mut target = json!({
        "name": "literal",
        "command": "echo hi",
        "foreach": items,
    });
    if let Some(expr) = when
        && let Some(map) = target.as_object_mut()
    {
        map.insert("when".into(), json!(expr));
    }
    json!({ "targets": [target] })
}

/// Strategy producing short lowercase item names.
fn item_names(max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-z]{1,8}", 0..=max)
}

/// Strategy producing items drawn from a keep/skip alphabet so `when`
/// expressions filter a meaningful subset.
fn keep_skip_items(max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(
        prop_oneof![Just("keep".to_owned()), Just("skip".to_owned())],
        0..=max,
    )
}

fn expanded_targets(doc: &ManifestValue) -> Result<&[ManifestValue], TestCaseError> {
    doc.get("targets")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .ok_or_else(|| TestCaseError::fail("targets sequence missing"))
}

proptest! {
    /// Absent `when` filtering, a list of `n` items expands to exactly `n`
    /// entries.
    #[test]
    fn expansion_yields_one_entry_per_item(items in item_names(10)) {
        let env = Environment::new();
        let mut doc = foreach_doc(&items, None);
        let stats = expand_foreach(&mut doc, &env)
            .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
        prop_assert_eq!(stats.filtered_targets, 0);
        let targets = expanded_targets(&doc)?;
        prop_assert_eq!(targets.len(), items.len());
    }

    /// `index` values across expanded entries are unique and form the
    /// sequence `0..n`.
    #[test]
    fn indexes_are_sequential_and_unique(items in item_names(10)) {
        let env = Environment::new();
        let mut doc = foreach_doc(&items, None);
        expand_foreach(&mut doc, &env)
            .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
        let targets = expanded_targets(&doc)?;
        let mut indexes = Vec::new();
        for target in targets {
            let index = target
                .get("vars")
                .and_then(|v| v.get("index"))
                .and_then(ManifestValue::as_u64)
                .ok_or_else(|| TestCaseError::fail("missing numeric index var"))?;
            indexes.push(index);
        }
        let expected: Vec<u64> = (0..items.len() as u64).collect();
        prop_assert_eq!(indexes, expected);
    }

    /// No expanded entry retains a `foreach` key.
    #[test]
    fn foreach_key_is_removed_from_all_entries(items in keep_skip_items(10)) {
        let env = Environment::new();
        let mut doc = foreach_doc(&items, Some("item == 'keep'"));
        expand_foreach(&mut doc, &env)
            .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
        for target in expanded_targets(&doc)? {
            let map = target
                .as_object()
                .ok_or_else(|| TestCaseError::fail("expanded entry is not an object"))?;
            prop_assert!(!map.contains_key("foreach"));
            prop_assert!(!map.contains_key("when"));
        }
    }

    /// Re-evaluating the same `when` expression on the same input always
    /// produces the same set of entries.
    #[test]
    fn when_filtering_is_deterministic(items in keep_skip_items(10)) {
        let env = Environment::new();
        let mut first = foreach_doc(&items, Some("item != 'skip'"));
        let mut second = first.clone();
        let first_stats = expand_foreach(&mut first, &env)
            .map_err(|e| TestCaseError::fail(format!("first expansion failed: {e}")))?;
        let second_stats = expand_foreach(&mut second, &env)
            .map_err(|e| TestCaseError::fail(format!("second expansion failed: {e}")))?;
        prop_assert_eq!(first_stats, second_stats);
        prop_assert_eq!(first, second);
    }

    /// Any `when` value composed solely of whitespace characters is rejected,
    /// regardless of whitespace kind or length.
    #[test]
    fn whitespace_only_when_is_rejected(
        ws in proptest::collection::vec(
            prop_oneof![Just(' '), Just('\t'), Just('\n'), Just('\r')],
            1..10,
        ),
        items in item_names(3),
    ) {
        let env = Environment::new();
        let expr: String = ws.into_iter().collect();
        let mut doc = foreach_doc(&items, Some(&expr));
        // The whitespace-only check applies on the iteration path and the
        // plain path alike; with zero items the `when` clause is never
        // reached, so anchor the non-iterating variant too.
        let mut plain = json!({
            "targets": [{ "name": "literal", "command": "echo hi", "when": expr }]
        });
        prop_assert!(expand_foreach(&mut plain, &env).is_err());
        if !items.is_empty() {
            prop_assert!(expand_foreach(&mut doc, &env).is_err());
        }
    }
}
