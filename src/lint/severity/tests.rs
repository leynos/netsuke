//! Tests for lint severity, defaults, and the failure threshold.

use rstest::rstest;

use super::{DefaultSeverity, FailOn, Severity, parse_policy_severity};

#[rstest]
#[case(Severity::Advice, "advice")]
#[case(Severity::Warning, "warning")]
#[case(Severity::Error, "error")]
fn severity_names_match_the_json_schema(#[case] severity: Severity, #[case] expected: &str) {
    assert_eq!(severity.as_str(), expected);
    assert_eq!(severity.to_string(), expected);
}

#[test]
fn severities_order_from_least_to_most_severe() {
    assert!(Severity::Advice < Severity::Warning);
    assert!(Severity::Warning < Severity::Error);
    assert_eq!(
        Severity::ALL,
        [Severity::Advice, Severity::Warning, Severity::Error]
    );
}

#[rstest]
#[case(FailOn::Never, Severity::Error, false)]
#[case(FailOn::Error, Severity::Error, true)]
#[case(FailOn::Error, Severity::Warning, false)]
#[case(FailOn::Warning, Severity::Warning, true)]
#[case(FailOn::Warning, Severity::Advice, false)]
#[case(FailOn::Advice, Severity::Advice, true)]
fn the_threshold_selects_the_severities_that_fail(
    #[case] threshold: FailOn,
    #[case] severity: Severity,
    #[case] expected: bool,
) {
    assert_eq!(threshold.is_reached_by(severity), expected);
}

#[test]
fn the_default_threshold_is_error() {
    assert_eq!(FailOn::default(), FailOn::Error);
}

#[rstest]
#[case("off", Ok(None))]
#[case("advice", Ok(Some(Severity::Advice)))]
#[case("warning", Ok(Some(Severity::Warning)))]
#[case("error", Ok(Some(Severity::Error)))]
#[case("ERROR", Err("ERROR"))]
#[case("fatal", Err("fatal"))]
fn policy_severities_parse_exactly(
    #[case] text: &str,
    #[case] expected: Result<Option<Severity>, &str>,
) {
    assert_eq!(parse_policy_severity(text), expected);
}

#[rstest]
#[case("advice", Ok(FailOn::Advice))]
#[case("never", Ok(FailOn::Never))]
#[case("loud", Err("loud".to_owned()))]
fn thresholds_parse_exactly(#[case] text: &str, #[case] expected: Result<FailOn, String>) {
    assert_eq!(text.parse::<FailOn>(), expected);
}

#[test]
fn an_opt_in_default_reports_no_severity() {
    assert_eq!(DefaultSeverity::Off.severity(), None);
    assert_eq!(DefaultSeverity::Off.as_str(), "off");
    assert_eq!(
        DefaultSeverity::On(Severity::Warning).severity(),
        Some(Severity::Warning)
    );
    assert_eq!(DefaultSeverity::On(Severity::Warning).as_str(), "warning");
}
