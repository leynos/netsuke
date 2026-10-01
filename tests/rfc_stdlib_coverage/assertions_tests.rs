//! Exercise each document-total and deny-set assertion independently.

use super::*;
use crate::rfc_stdlib_coverage::validation_fixtures as fixture;
use rstest::rstest;

#[test]
fn accepts_consistent_totals_and_complement() -> Result<()> {
    check_against_document(&fixture::survey())?;
    check_denied(&fixture::survey())
}

#[rstest]
#[case::accept("accept", "derived 1 accept rows")]
#[case::defer("defer", "derived 0 defer rows")]
#[case::reject("reject", "table 11's classes [1, 0, 0] sum to 1")]
#[case::filters("filters", "derived 1 new filters")]
#[case::tests("tests", "derived 0 new tests")]
#[case::proposed("proposed", "derived 1 new helpers")]
#[case::optioned("optioned", "derived 0 existing helpers gaining an option")]
#[case::accepted("accepted", "accepted set has 0 members")]
fn rejects_inconsistent_document_totals(#[case] mutation: &str, #[case] diagnostic: &str) {
    let mut survey = fixture::survey();
    match mutation {
        "accept" => survey.stated_accept += 1,
        "defer" => survey.stated_defer += 1,
        "reject" => {
            *survey
                .reject_classes
                .first_mut()
                .expect("fixture reject class") += 1;
        }
        "filters" => survey.new_filters += 1,
        "tests" => survey.new_tests += 1,
        "proposed" => survey.proposed += 1,
        "optioned" => survey.optioned_total += 1,
        "accepted" => survey.accepted.clear(),
        _ => {}
    }
    let error = check_against_document(&survey).expect_err("one total changed");
    assert!(error.to_string().contains(diagnostic), "{error:#}");
}

#[rstest]
#[case::count("count", "derived 70 forbidden names")]
#[case::missing("missing", "deny set omits [\"is_file\"]")]
#[case::accepted("accepted", "deny set forbids [\"basename\"]")]
#[case::hash_accepted("hash_accepted", "it is in the accepted set")]
#[case::hash_denied("hash_denied", "it is in the deny set")]
#[case::hash_both("hash_both", "it is in the accepted set and the deny set")]
fn rejects_invalid_complement(#[case] mutation: &str, #[case] diagnostic: &str) {
    let mut survey = fixture::survey();
    match mutation {
        "count" => {
            survey.denied.remove("denied0");
        }
        "missing" => {
            survey.denied.remove("is_file");
            survey.denied.insert("replacement".into());
        }
        "accepted" => {
            survey.denied.remove("denied0");
            survey.denied.insert("basename".into());
        }
        "hash_accepted" => {
            survey
                .accepted
                .insert("hash".into(), fixture::helper("hash"));
        }
        "hash_denied" => {
            survey.denied.remove("denied0");
            survey.denied.insert("hash".into());
        }
        "hash_both" => {
            survey
                .accepted
                .insert("hash".into(), fixture::helper("hash"));
            survey.denied.remove("denied0");
            survey.denied.insert("hash".into());
        }
        _ => {}
    }
    let error = check_denied(&survey).expect_err("one complement condition changed");
    assert!(error.to_string().contains(diagnostic), "{error:#}");
}
