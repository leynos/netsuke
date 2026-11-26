//! `OBL-STATIC-WHEN` (`RM-4.3.2.d`, clause `C-4`).
//!
//! A static entry survives exactly when it has no condition or its condition
//! holds over its own `vars`. The two sections' filtered counts equal the
//! oracle's exclusion counts, and every retained record for a static entry
//! reports no iteration index.

use super::model::expected_filtered;
use super::strategy::{Section, static_entry_spec_strategy};
use super::support::{expand_document, section_entries};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn static_entries_survive_exactly_when_their_condition_holds(
        spec in static_entry_spec_strategy()
    ) {
        let expected_kept = usize::from(expected_filtered(&spec.body).is_empty());
        let mut document = spec.render_document();

        let report = expand_document(&mut document)?;

        let actual = section_entries(&document, spec.section.name())?;
        prop_assert_eq!(actual.len(), expected_kept, "kept entry count");
        let expected_for_section = expected_filtered(&spec.body).len();
        let (filtered_targets, filtered_actions) = match spec.section {
            Section::Targets => (expected_for_section, 0),
            Section::Actions => (0, expected_for_section),
        };
        prop_assert_eq!(report.stats.filtered_targets, filtered_targets, "filtered targets");
        prop_assert_eq!(report.stats.filtered_actions, filtered_actions, "filtered actions");
        let retained: Vec<&crate::manifest::expand::FilteredEntry> = report
            .filtered_entries
            .iter()
            .filter(|entry| entry.section == spec.section.name())
            .collect();
        prop_assert_eq!(retained.len(), expected_for_section, "retained records");
        for entry in retained {
            prop_assert_eq!(entry.iteration_index, None, "static entries report no index");
        }
    }
}
