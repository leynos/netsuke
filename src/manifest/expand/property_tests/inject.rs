//! Prove each `foreach` iteration injects `item` and `index` (`OBL-INJECT`,
//! `RM-4.3.2.c`, `C-3`) via `expand_foreach`, `support` and `model`.

use super::model::deep_equal;
use super::strategy::foreach_entry_spec_strategy;
use super::support::{expand_document, section_entries, vars_of};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn each_expansion_carries_its_own_item_and_index(spec in foreach_entry_spec_strategy()) {
        let items = spec.body.items().ok_or_else(|| {
            TestCaseError::fail("the foreach generator produced a static entry")
        })?;
        let expected_indexes: Vec<usize> = (0..items.len())
            .filter(|index| spec.body.condition_holds(items.value_at(*index).as_i64(), *index))
            .collect();
        let mut document = spec.render_document();

        expand_document(&mut document)?;

        let actual = section_entries(&document, spec.section.name())?;
        prop_assert_eq!(
            actual.len(),
            expected_indexes.len(),
            "expanded entry count"
        );
        for (entry, index) in actual.iter().zip(&expected_indexes) {
            let vars = vars_of(entry).ok_or_else(|| {
                TestCaseError::fail(format!("expanded entry has no vars: {entry}"))
            })?;
            let item = vars.get("item").ok_or_else(|| {
                TestCaseError::fail("expanded vars has no `item`")
            })?;
            prop_assert!(
                deep_equal(item, &items.value_at(*index)),
                "vars.item is {} but iteration {} is {}",
                item,
                index,
                items.value_at(*index)
            );
            prop_assert_eq!(
                vars.get("index").and_then(serde_json::Value::as_u64),
                Some(*index as u64),
                "vars.index"
            );
            for key in spec.body.vars_keys() {
                if matches!(key, "item" | "index") {
                    continue;
                }
                let actual_value = vars.get(key).ok_or_else(|| {
                    TestCaseError::fail(format!("user vars key `{key}` disappeared"))
                })?;
                let authored = spec.body.vars_value(key).ok_or_else(|| {
                    TestCaseError::fail(format!("model lost user vars key `{key}`"))
                })?;
                prop_assert!(
                    deep_equal(actual_value, authored),
                    "user vars `{}` changed: {} != {}",
                    key,
                    actual_value,
                    authored
                );
            }
        }
    }
}
