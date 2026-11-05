//! Registry composition rules at the merged-sequence boundary.
//!
//! Each case states the merged sequence a pair of layers could produce, so a
//! test failure names the offending position rather than an abstract property.

use super::super::{CompositionError, validate_composition};
use super::definitions;
use rstest::rstest;

/// Accept registries with no reserved name and no repeated name.
#[rstest]
#[case::empty(&[])]
#[case::one_name(&["dash"])]
#[case::distinct_names(&["dash", "fish", "nu"])]
fn admissible_registries_are_accepted(#[case] names: &[&str]) {
    assert_eq!(
        validate_composition(&definitions(names)),
        Ok(()),
        "{names:?} must be accepted"
    );
}

/// Reject a definition that reuses one of the four built-in spellings.
#[rstest]
#[case::sh("sh")]
#[case::bash("bash")]
#[case::pwsh("pwsh")]
#[case::powershell("powershell")]
fn built_in_names_are_rejected(#[case] name: &str) {
    assert_eq!(
        validate_composition(&definitions(&["dash", name])),
        Err(CompositionError::Reserved {
            index: 1,
            name: name.to_owned(),
        })
    );
}

/// Reject a name declared twice, whether adjacent or separated.
#[rstest]
#[case::adjacent(&["dash", "dash"], 0)]
#[case::separated(&["dash", "fish", "dash"], 0)]
fn repeated_names_are_rejected(#[case] names: &[&str], #[case] first: usize) {
    let index = names.len() - 1;
    let name = names
        .last()
        .expect("every case declares at least one name")
        .to_string();
    assert_eq!(
        validate_composition(&definitions(names)),
        Err(CompositionError::Duplicate { index, first, name })
    );
}

/// Report a reserved name before reporting the collision it also causes.
#[test]
fn a_reserved_repeat_reports_the_reserved_rule() {
    assert_eq!(
        validate_composition(&definitions(&["sh", "sh"])),
        Err(CompositionError::Reserved {
            index: 0,
            name: "sh".to_owned(),
        })
    );
}

/// Name the position and spelling so a diagnostic points at one definition.
#[test]
fn a_duplicate_message_names_both_positions() {
    let error = validate_composition(&definitions(&["dash", "fish", "dash"]))
        .expect_err("a repeated name is rejected");
    let message = error.to_string();
    assert!(
        message.contains("shells[2]") && message.contains("shells[0]") && message.contains("dash"),
        "the message must locate both positions: {message}"
    );
}
