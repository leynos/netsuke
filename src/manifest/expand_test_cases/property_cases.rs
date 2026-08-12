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

/// Build a single-section manifest document iterating over `items`.
fn foreach_doc(section: &str, items: &[String], when: Option<&str>) -> ManifestValue {
    let mut entry = json!({
        "name": "literal",
        "description": "Build {{ item }}",
        "command": "echo hi",
        "foreach": items,
    });
    if let Some(expr) = when
        && let Some(map) = entry.as_object_mut()
    {
        map.insert("when".into(), json!(expr));
    }
    let mut doc = ManifestMap::new();
    doc.insert(section.into(), json!([entry]));
    ManifestValue::Object(doc)
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

fn expanded_entries<'a>(
    doc: &'a ManifestValue,
    section: &str,
) -> Result<&'a [ManifestValue], TestCaseError> {
    doc.get(section)
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .ok_or_else(|| TestCaseError::fail(format!("{section} sequence missing")))
}

proptest! {
    /// Absent `when` filtering, a list of `n` items expands to exactly `n`
    /// entries.
    #[test]
    fn expansion_yields_one_entry_per_item(items in item_names(10)) {
        let env = Environment::new();
        for section in ["targets", "actions"] {
            let mut doc = foreach_doc(section, &items, None);
            let stats = expand_foreach(&mut doc, &env)
                .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
            prop_assert_eq!(stats.filtered_targets, 0);
            prop_assert_eq!(stats.filtered_actions, 0);
            let entries = expanded_entries(&doc, section)?;
            prop_assert_eq!(entries.len(), items.len());
        }
    }

    /// `index` values across expanded entries are unique and form the
    /// sequence `0..n`.
    #[test]
    fn indexes_are_sequential_and_unique(items in item_names(10)) {
        let env = Environment::new();
        for section in ["targets", "actions"] {
            let mut doc = foreach_doc(section, &items, None);
            expand_foreach(&mut doc, &env)
                .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
            let entries = expanded_entries(&doc, section)?;
            let mut indexes = Vec::new();
            for entry in entries {
                let index = entry
                    .get("vars")
                    .and_then(|v| v.get("index"))
                    .and_then(ManifestValue::as_u64)
                    .ok_or_else(|| TestCaseError::fail("missing numeric index var"))?;
                indexes.push(index);
            }
            let expected: Vec<u64> = (0..items.len() as u64).collect();
            prop_assert_eq!(indexes, expected);
        }
    }

    /// Every `foreach` clone keeps its discovery metadata so final rendering
    /// can resolve the same item-specific description as the target name.
    #[test]
    fn foreach_preserves_description_templates(items in item_names(10)) {
        let env = Environment::new();
        for section in ["targets", "actions"] {
            let mut doc = foreach_doc(section, &items, None);
            expand_foreach(&mut doc, &env)
                .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
            let descriptions: Result<Vec<_>, TestCaseError> = expanded_entries(&doc, section)?
                .iter()
                .map(|entry| {
                    entry
                        .get("description")
                        .and_then(ManifestValue::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| TestCaseError::fail("expanded description missing"))
                })
                .collect();
            prop_assert_eq!(
                descriptions?,
                vec!["Build {{ item }}".to_owned(); items.len()]
            );
        }
    }

    /// No expanded entry retains a `foreach` key.
    #[test]
    fn foreach_key_is_removed_from_all_entries(items in keep_skip_items(10)) {
        let env = Environment::new();
        for section in ["targets", "actions"] {
            let mut doc = foreach_doc(section, &items, Some("item == 'keep'"));
            expand_foreach(&mut doc, &env)
                .map_err(|e| TestCaseError::fail(format!("expansion failed: {e}")))?;
            for entry in expanded_entries(&doc, section)? {
                let map = entry
                    .as_object()
                    .ok_or_else(|| TestCaseError::fail("expanded entry is not an object"))?;
                prop_assert!(!map.contains_key("foreach"));
                prop_assert!(!map.contains_key("when"));
            }
        }
    }

    /// Re-evaluating the same `when` expression on the same input always
    /// produces the same set of entries.
    #[test]
    fn when_filtering_is_deterministic(items in keep_skip_items(10)) {
        let env = Environment::new();
        for section in ["targets", "actions"] {
            let mut first = foreach_doc(section, &items, Some("item != 'skip'"));
            let mut second = first.clone();
            let first_stats = expand_foreach(&mut first, &env)
                .map_err(|e| TestCaseError::fail(format!("first expansion failed: {e}")))?;
            let second_stats = expand_foreach(&mut second, &env)
                .map_err(|e| TestCaseError::fail(format!("second expansion failed: {e}")))?;
            prop_assert_eq!(first_stats, second_stats);
            prop_assert_eq!(first, second);
        }
    }

    /// Any `when` value composed solely of whitespace characters is rejected,
    /// regardless of whitespace kind or length.
    #[test]
    fn whitespace_only_when_is_rejected(
        ws in proptest::collection::vec(
            prop::sample::select(vec![
                '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}',
                '\u{0085}', '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}',
                '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}',
                '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}',
                '\u{3000}',
            ]),
            1..10,
        ),
        items in item_names(3),
    ) {
        let env = Environment::new();
        let expr: String = ws.into_iter().collect();
        for section in ["targets", "actions"] {
            let mut doc = foreach_doc(section, &items, Some(&expr));
            // The whitespace-only check applies on the iteration path and the
            // plain path alike; with zero items the `when` clause is never
            // reached, so anchor the non-iterating variant too.
            let mut plain_section = ManifestMap::new();
            plain_section.insert(
                section.into(),
                json!([{ "name": "literal", "command": "echo hi", "when": expr.as_str() }]),
            );
            let mut plain = ManifestValue::Object(plain_section);
            prop_assert!(expand_foreach(&mut plain, &env).is_err());
            if !items.is_empty() {
                prop_assert!(expand_foreach(&mut doc, &env).is_err());
            }
        }
    }
}
