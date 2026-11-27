//! Check non-control fields survive expansion unchanged (`OBL-PRESERVE`,
//! `RM-4.3.2.a`, `C-1`), comparing `support` with `model::expected_outputs`.

use super::model::{deep_equal, expected_outputs};
use super::strategy::entry_spec_strategy;
use super::support::{entry_map, expand_document, key_sequence, section_entries, vars_of};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn expansion_copies_every_non_control_field(spec in entry_spec_strategy()) {
        let expected = expected_outputs(&spec);
        let mut document = spec.render_document();

        expand_document(&mut document)?;

        let actual = section_entries(&document, spec.section.name())?;
        prop_assert_eq!(actual.len(), expected.len(), "entry count");
        for (actual_entry, expected_entry) in actual.iter().zip(&expected) {
            let actual_map = entry_map(actual_entry)?;
            let expected_map = entry_map(expected_entry)?;
            prop_assert_eq!(
                key_sequence(actual_map),
                key_sequence(expected_map),
                "entry key sequence"
            );
            for (key, value) in spec.body.fields() {
                let actual_value = actual_map.get(key).ok_or_else(|| {
                    TestCaseError::fail(format!("field `{key}` missing from the expanded entry"))
                })?;
                prop_assert!(
                    deep_equal(actual_value, value),
                    "field `{}` changed: {} != {}",
                    key,
                    actual_value,
                    value
                );
            }
            if spec.body.has_vars() || spec.body.is_foreach() {
                let actual_vars = vars_of(actual_entry).ok_or_else(|| {
                    TestCaseError::fail("expanded entry does not carry vars")
                })?;
                let expected_vars = vars_of(expected_entry).ok_or_else(|| {
                    TestCaseError::fail("model does not predict vars")
                })?;
                prop_assert_eq!(
                    key_sequence(actual_vars),
                    key_sequence(expected_vars),
                    "vars key sequence"
                );
                for key in spec.body.vars_keys() {
                    if spec.body.is_foreach() && matches!(key, "item" | "index") {
                        continue;
                    }
                    let actual_value = actual_vars.get(key).ok_or_else(|| {
                        TestCaseError::fail(format!("vars key `{key}` missing"))
                    })?;
                    let expected_value = expected_vars.get(key).ok_or_else(|| {
                        TestCaseError::fail(format!("model omits vars key `{key}`"))
                    })?;
                    prop_assert!(
                        deep_equal(actual_value, expected_value),
                        "vars `{}` changed: {} != {}",
                        key,
                        actual_value,
                        expected_value
                    );
                }
            }
        }
    }
}
