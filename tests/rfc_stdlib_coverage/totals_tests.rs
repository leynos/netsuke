//! Pin tally rejection diagnostics and guarded purity evidence.
use super::*;
use rstest::rstest;

/// Build the smallest complete tally table.
fn tally() -> String {
    let labels = [
        "Surveyed entries accepted",
        "Surveyed entries deferred",
        "Surveyed entries rejected because",
        "Surveyed entries rejected as a redundant alias",
        "Surveyed entries rejected on principle",
        "New Netsuke filters introduced",
        "New Netsuke tests introduced",
        "Existing Netsuke helpers gaining",
    ];
    let rows = labels
        .into_iter()
        .map(|label| format!("| {label} | 1 |\n"))
        .collect::<Vec<_>>()
        .concat();
    format!("### 7.8. Totals\n| Measure | Count |\n|---|---|\n{rows}")
}

#[test]
fn complete_tallies_accept_comma_separated_counts() {
    let text = tally().replace("accepted | 1", "accepted | 1,234");
    let totals = table_11(&Section::whole(&text)).expect("valid tally");
    assert_eq!(totals.accept_rows, 1234);
    assert_eq!(totals.reject_classes, [1, 1, 1]);
}

#[rstest]
#[case::missing(
    "| Surveyed entries accepted | 1 |\n",
    "",
    "expected exactly one table 11 row"
)]
#[case::duplicate(
    "| Surveyed entries accepted | 1 |",
    "| Surveyed entries accepted | 1 |\n| Surveyed entries accepted | 2 |",
    "twice"
)]
#[case::ambiguous(
    "| Surveyed entries accepted | 1 |",
    "| Surveyed entries accepted | 1 |\n| Surveyed entries accepted extra | 2 |",
    "expected exactly one table 11 row"
)]
#[case::not_numeric("accepted | 1", "accepted | many", "is not a number")]
#[case::missing_count(
    "| Surveyed entries accepted | 1 |",
    "| Surveyed entries accepted |",
    "row at line 4 has no count column"
)]
fn tally_rejections_are_specific(#[case] old: &str, #[case] new: &str, #[case] diagnostic: &str) {
    let text = tally().replace(old, new);
    let error = table_11(&Section::whole(&text))
        .err()
        .expect("invalid tally");
    assert!(format!("{error:#}").contains(diagnostic), "{error:#}");
}

#[rstest]
#[case::missing_heading("## Other", "RFC 0006 has no section 6.1")]
#[case::changed_evidence(
    "### 6.1. Purity classes\nDifferent aggregate",
    "no longer states the purity aggregate"
)]
fn purity_requires_unchanged_evidence(#[case] text: &str, #[case] diagnostic: &str) {
    let error = purity_aggregate(&Section::whole(text)).expect_err("missing evidence");
    assert!(error.to_string().contains(diagnostic));
}

#[test]
fn purity_evidence_allows_line_wrapping() {
    let text = format!(
        "### 6.1. Purity classes\n{}",
        PURITY_SENTENCE.replace("pure, ", "pure,\n")
    );
    assert_eq!(
        purity_aggregate(&Section::whole(&text)).expect("same evidence"),
        (52, 4, 1)
    );
}
