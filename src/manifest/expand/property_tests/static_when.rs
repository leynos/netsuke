//! `OBL-STATIC-WHEN` (`RM-4.3.2.d`, clause `C-4`).

use super::model::{expected_document, expected_filtered};
use super::strategy::{DocumentSpec, static_document_spec_strategy};
use super::support::expand_document;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn static_entries_survive_exactly_when_their_condition_holds(
        spec in static_document_spec_strategy()
    ) {
        let mut document = spec.render();

        let report = expand_document(&mut document)?;

        prop_assert_eq!(
            document,
            crate::manifest::ManifestValue::Object(expected_document(&spec)),
            "expanded document"
        );
        check_filtered_counts(&spec, &report)?;
        check_retained_records(&spec, &report)?;
    }
}

/// Verify the aggregate filtered counts equal the oracle's exclusions.
fn check_filtered_counts(
    spec: &DocumentSpec,
    report: &crate::manifest::expand::ExpansionReport,
) -> Result<(), TestCaseError> {
    let (targets, actions) = expected_exclusions(spec);
    prop_assert_eq!(report.stats.filtered_targets, targets, "filtered targets");
    prop_assert_eq!(report.stats.filtered_actions, actions, "filtered actions");
    Ok(())
}

/// Count the exclusions the oracle predicts for each section.
fn expected_exclusions(spec: &DocumentSpec) -> (usize, usize) {
    let targets = spec
        .targets
        .iter()
        .map(|entry| expected_filtered(&entry.body).len())
        .sum();
    let actions = spec
        .actions
        .iter()
        .map(|entry| expected_filtered(&entry.body).len())
        .sum();
    (targets, actions)
}

/// Verify every retained record is a static entry's, in both sections.
fn check_retained_records(
    spec: &DocumentSpec,
    report: &crate::manifest::expand::ExpansionReport,
) -> Result<(), TestCaseError> {
    let (targets, actions) = expected_exclusions(spec);
    prop_assert_eq!(
        report.omitted_filtered_entries,
        0,
        "a document of this size retains every record"
    );
    prop_assert_eq!(
        report.filtered_entries.len(),
        targets + actions,
        "record count"
    );
    let mut seen_targets = 0;
    let mut seen_actions = 0;
    for entry in &report.filtered_entries {
        prop_assert_eq!(
            entry.iteration_index,
            None,
            "static entries report no index"
        );
        match entry.section.as_str() {
            "targets" => seen_targets += 1,
            "actions" => seen_actions += 1,
            other => {
                return Err(TestCaseError::fail(format!(
                    "unexpected filtered-entry section `{other}`"
                )));
            }
        }
    }
    prop_assert_eq!(seen_targets, targets, "targets records");
    prop_assert_eq!(seen_actions, actions, "actions records");
    Ok(())
}
