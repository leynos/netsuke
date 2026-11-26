//! `OBL-ORDER` (`FV-EXP`, `DES-ORDER`, clause `C-2`).
//!
//! An expanded entry's key sequence is the authored sequence with the control
//! keys deleted, keeping the relative order of every other key, and the `vars`
//! sequence keeps authored keys in place while appending the injected ones.
//! The two properties separate the two removal sites: the first exercises
//! `foreach` only, the second `when` only.

use super::model::expected_outputs;
use super::strategy::{
    EntrySpec, foreach_entry_spec_strategy, static_condition_entry_spec_strategy,
};
use super::support::{
    entry_map, expand_document, key_sequence, owned_key_sequence, section_entries, vars_of,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn foreach_removal_keeps_authoring_key_order(spec in foreach_entry_spec_strategy()) {
        let spec = EntrySpec {
            section: spec.section,
            body: spec.body.without_condition(),
        };
        let expected = expected_outputs(&spec);
        let mut document = spec.render_document();

        expand_document(&mut document)?;

        let actual = section_entries(&document, spec.section.name())?;
        prop_assert_eq!(actual.len(), expected.len(), "entry count");
        for entry in actual {
            let map = entry_map(entry)?;
            prop_assert_eq!(
                key_sequence(map),
                spec.body.predicted_keys(),
                "entry key sequence"
            );
            let actual_vars = vars_of(entry).map(owned_key_sequence);
            prop_assert_eq!(
                actual_vars,
                spec.body.predicted_vars_keys(),
                "vars key sequence"
            );
        }
    }

    #[test]
    fn when_removal_keeps_authoring_key_order(
        spec in static_condition_entry_spec_strategy()
    ) {
        let expected = expected_outputs(&spec);
        let mut document = spec.render_document();

        expand_document(&mut document)?;

        let actual = section_entries(&document, spec.section.name())?;
        prop_assert_eq!(actual.len(), expected.len(), "entry count");
        for entry in actual {
            let map = entry_map(entry)?;
            prop_assert_eq!(
                key_sequence(map),
                spec.body.predicted_keys(),
                "entry key sequence"
            );
            let actual_vars = vars_of(entry).map(owned_key_sequence);
            prop_assert_eq!(
                actual_vars,
                spec.body.predicted_vars_keys(),
                "vars key sequence"
            );
        }
    }
}
