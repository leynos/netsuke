//! Generated invariants for manifest foreach expansion.
//!
//! The fixed cases pin specific regressions, while these properties vary
//! sequence values, filtering, variable collisions, and source key order.

use super::*;
use crate::manifest::ManifestBudgetLimits;
use minijinja::Environment;
use proptest::prelude::*;

proptest! {
    /// A bounded foreach consumes no more entries than its configured ceiling.
    #[test]
    fn foreach_budget_terminates_deterministically_at_the_cardinality_limit(
        values in proptest::collection::vec(-100_i16..100, 0..12),
    ) {
        const CARDINALITY_LIMIT: usize = 4;
        let env = Environment::new();
        let yaml = format!(
            "targets:\n  - name: generated\n    foreach: {values:?}\n    command: echo {{{{ item }}}}"
        );
        let doc: ManifestValue = serde_saphyr::from_str(&yaml)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let limits = ManifestBudgetLimits {
            foreach_cardinality: CARDINALITY_LIMIT,
            expanded_entries: CARDINALITY_LIMIT,
            ..ManifestBudgetLimits::default()
        };

        let mut first = doc.clone();
        let first_budget = ManifestBudget::new(limits)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let first_result = expand_foreach_with_budget(&mut first, &env, &first_budget);
        let mut second = doc;
        let second_budget = ManifestBudget::new(limits)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let second_result = expand_foreach_with_budget(&mut second, &env, &second_budget);

        if values.len() <= CARDINALITY_LIMIT {
            first_result.map_err(|error| TestCaseError::fail(error.to_string()))?;
            second_result.map_err(|error| TestCaseError::fail(error.to_string()))?;
            let entries = targets(&first)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(entries.len(), values.len());
            prop_assert_eq!(first, second);
        } else {
            let first_error = first_result.expect_err("over-limit foreach must fail");
            let second_error = second_result.expect_err("over-limit foreach must fail");
            prop_assert!(first_error.to_string().contains("resource budget exhausted"));
            prop_assert_eq!(first_error.to_string(), second_error.to_string());
            prop_assert_eq!(first, second);
        }
    }

    /// Expansion preserves a leading non-object entry and expands each value.
    #[test]
    fn foreach_expands_generated_sequence_values(values in proptest::collection::vec(-100_i16..100, 1..8)) {
        let env = Environment::new();
        let yaml = format!(
            "targets:\n  - bare-string\n  - name: generated\n    foreach: {values:?}\n    command: echo {{{{ item }}}}"
        );
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        expand_foreach(&mut doc, &env)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expanded = targets(&doc).map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(expanded.len(), values.len() + 1);
        prop_assert_eq!(expanded.first().and_then(ManifestValue::as_str), Some("bare-string"));
        for (index, (entry, value)) in expanded.iter().skip(1).zip(&values).enumerate() {
            let map = entry.as_object().ok_or_else(|| {
                TestCaseError::fail(format!("expanded entry {index} should be an object: {entry:?}"))
            })?;
            prop_assert!(!map.contains_key("foreach"));
            prop_assert_eq!(map.get("name").and_then(ManifestValue::as_str), Some("generated"));
            let vars = map.get("vars").and_then(ManifestValue::as_object).ok_or_else(|| {
                TestCaseError::fail(format!("expanded entry {index} should contain vars: {map:?}"))
            })?;
            prop_assert_eq!(vars.get("item").and_then(ManifestValue::as_i64), Some(i64::from(*value)));
            prop_assert_eq!(vars.get("index").and_then(ManifestValue::as_u64), Some(index as u64));
        }
    }

    /// Iteration values override colliding entry vars while other vars survive.
    #[test]
    fn foreach_generated_iteration_values_override_entry_vars(
        values in proptest::collection::vec("[a-z]{1,6}", 1..8),
        entry_item in "[a-z]{1,6}",
        other_value in "[a-z]{1,6}",
    ) {
        let env = Environment::new();
        let foreach = serde_json::to_string(&values)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let entry_item_json = serde_json::to_string(&entry_item)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let other = serde_json::to_string(&other_value)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let yaml = format!(
            "targets:\n  - name: variables\n    foreach: {foreach}\n    vars:\n      item: {entry_item_json}\n      other: {other}"
        );
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        expand_foreach(&mut doc, &env)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expanded = targets(&doc).map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(expanded.len(), values.len());
        for (index, (entry, value)) in expanded.iter().zip(&values).enumerate() {
            let map = entry.as_object().ok_or_else(|| {
                TestCaseError::fail(format!("expanded entry {index} should be an object: {entry:?}"))
            })?;
            let vars = map.get("vars").and_then(ManifestValue::as_object).ok_or_else(|| {
                TestCaseError::fail(format!("expanded entry {index} should contain vars: {map:?}"))
            })?;
            prop_assert_eq!(vars.get("item").and_then(ManifestValue::as_str), Some(value.as_str()));
            prop_assert_eq!(vars.get("other").and_then(ManifestValue::as_str), Some(other_value.as_str()));
            prop_assert_eq!(vars.get("index").and_then(ManifestValue::as_u64), Some(index as u64));
        }
    }

    /// Filtering preserves original indexes for every generated input sequence.
    #[test]
    fn foreach_filters_generated_sequence_values(
        values in proptest::collection::vec(-20_i16..21, 0..8),
        threshold in -20_i16..21,
    ) {
        let env = Environment::new();
        let yaml = format!(
            "targets:\n  - name: filtered\n    foreach: {values:?}\n    when: 'item > {threshold}'"
        );
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        expand_foreach(&mut doc, &env)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expanded = targets(&doc).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expected: Vec<_> = values
            .iter()
            .enumerate()
            .filter(|(_, value)| **value > threshold)
            .collect();

        prop_assert_eq!(expanded.len(), expected.len());
        for (entry, (index, value)) in expanded.iter().zip(expected) {
            let map = entry.as_object().ok_or_else(|| {
                TestCaseError::fail(format!("filtered entry should be an object: {entry:?}"))
            })?;
            prop_assert!(!map.contains_key("foreach"));
            let vars = map.get("vars").and_then(ManifestValue::as_object).ok_or_else(|| {
                TestCaseError::fail(format!("filtered entry should contain vars: {map:?}"))
            })?;
            prop_assert_eq!(vars.get("item").and_then(ManifestValue::as_i64), Some(i64::from(*value)));
            prop_assert_eq!(vars.get("index").and_then(ManifestValue::as_u64), Some(index as u64));
        }
    }

    /// Expansion removes `foreach` without reordering user-specified map keys.
    #[test]
    fn foreach_preserves_generated_source_key_order(order in prop_oneof![
        Just(vec!["name", "vars", "after"]),
        Just(vec!["name", "after", "vars"]),
        Just(vec!["vars", "name", "after"]),
        Just(vec!["vars", "after", "name"]),
        Just(vec!["after", "name", "vars"]),
        Just(vec!["after", "vars", "name"]),
    ]) {
        let env = Environment::new();
        let mut yaml = String::from("targets:\n");
        for (index, key) in order.iter().enumerate() {
            yaml.push_str(if index == 0 { "  - " } else { "    " });
            match *key {
                "name" => yaml.push_str("name: ordered\n"),
                "vars" => yaml.push_str("vars:\n      static: keep\n"),
                "after" => yaml.push_str("after: done\n"),
                _ => return Err(TestCaseError::fail("property strategy produced an unknown key".to_owned())),
            }
        }
        yaml.push_str("    foreach: [value]");
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        expand_foreach(&mut doc, &env)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expanded = targets(&doc).map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(expanded.len(), 1);
        let entry = expanded.first().ok_or_else(|| {
            TestCaseError::fail("expected one expanded target after length check".to_owned())
        })?;
        let map = entry.as_object().ok_or_else(|| {
            TestCaseError::fail(format!("expanded target should be an object: {entry:?}"))
        })?;
        let keys: Vec<_> = map.keys().map(String::as_str).collect();
        prop_assert_eq!(keys, order);
    }
}
