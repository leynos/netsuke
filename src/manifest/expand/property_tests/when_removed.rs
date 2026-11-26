//! `OBL-WHEN-REMOVED` (`RM-4.3.2.b`, clause `C-4`).
//!
//! No expanded entry carries a control key, whatever mix of static and
//! `foreach` entries, conditions, and sections the document holds, and the
//! filtered counts equal the number of entries the oracle says are excluded.

use super::model::{expected_document, expected_filtered};
use super::strategy::{Section, document_spec_strategy};
use super::support::{entry_map, expand_document, section_entries};
use proptest::prelude::*;

proptest! {
    #[test]
    fn no_expanded_entry_carries_a_control_key(spec in document_spec_strategy()) {
        let mut document = spec.render();

        let report = expand_document(&mut document)?;

        let mut expected_targets = 0;
        let mut expected_actions = 0;
        for section in [Section::Targets, Section::Actions] {
            for entry in spec.entries(section) {
                let excluded = expected_filtered(&entry.body).len();
                match section {
                    Section::Targets => expected_targets += excluded,
                    Section::Actions => expected_actions += excluded,
                }
            }
            for entry in section_entries(&document, section.name())? {
                let map = entry_map(entry)?;
                prop_assert!(
                    !map.contains_key("when"),
                    "expanded {} entry still carries `when`: {entry}",
                    section.name()
                );
                prop_assert!(
                    !map.contains_key("foreach"),
                    "expanded {} entry still carries `foreach`: {entry}",
                    section.name()
                );
            }
        }
        prop_assert_eq!(report.stats.filtered_targets, expected_targets, "filtered targets");
        prop_assert_eq!(report.stats.filtered_actions, expected_actions, "filtered actions");
        prop_assert_eq!(
            document,
            crate::manifest::ManifestValue::Object(expected_document(&spec)),
            "expanded document"
        );
    }
}
