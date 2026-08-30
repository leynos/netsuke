//! Tests for report bounding and summarizing.

use rstest::rstest;

use super::{Bounds, NamedManifest, Report};
use crate::lint::engine::Outcome;
use crate::lint::finding::Finding;
use crate::lint::registry;
use crate::lint::severity::{FailOn, Severity};

/// Build an outcome holding one finding per entry of `severities`.
macro_rules! outcome {
    ($severities:expr $(,)?) => {
        build_outcome(
            registry::meta_by_name("background-job").expect("the rule should be registered"),
            $severities,
        )
    };
}

/// Build an outcome attributing every finding to `meta`.
fn build_outcome(meta: &'static crate::lint::rule::RuleMeta, severities: &[Severity]) -> Outcome {
    Outcome {
        findings: severities
            .iter()
            .enumerate()
            .map(|(index, severity)| {
                Finding::detached(meta, *severity, "detached", format!("target `{index}`"))
            })
            .collect(),
        suppressed: 2,
    }
}

/// Build a report over some severities, bounded to a limit.
macro_rules! report {
    ($severities:expr, $limit:expr, $threshold:expr $(,)?) => {
        Report::new(
            NamedManifest {
                name: "Netsukefile",
                source: "netsuke_version: \"1.0.0\"\n".to_owned(),
            },
            outcome!($severities),
            Bounds {
                limit: $limit,
                threshold: $threshold,
            },
        )
    };
}

#[test]
fn a_report_counts_findings_by_severity() {
    let built = report!(
        &[Severity::Error, Severity::Warning, Severity::Warning],
        0,
        FailOn::Error,
    );
    assert_eq!(built.count_at(Severity::Error), 1);
    assert_eq!(built.count_at(Severity::Warning), 2);
    assert_eq!(built.count_at(Severity::Advice), 0);
    assert_eq!(built.suppressed(), 2);
}

#[rstest]
#[case(0, 3, 0)]
#[case(5, 3, 0)]
#[case(2, 2, 1)]
#[case(1, 1, 2)]
fn a_limit_bounds_the_report_and_records_what_it_dropped(
    #[case] limit: usize,
    #[case] reported: usize,
    #[case] truncated: usize,
) {
    let built = report!(
        &[Severity::Error, Severity::Warning, Severity::Advice],
        limit,
        FailOn::Error,
    );
    assert_eq!(built.findings().len(), reported);
    assert_eq!(built.truncated(), truncated);
}

#[rstest]
#[case(FailOn::Error, 1, true)]
#[case(FailOn::Warning, 2, true)]
#[case(FailOn::Advice, 3, true)]
#[case(FailOn::Never, 0, false)]
fn the_threshold_decides_the_verdict(
    #[case] threshold: FailOn,
    #[case] failing: usize,
    #[case] is_failure: bool,
) {
    let built = report!(
        &[Severity::Error, Severity::Warning, Severity::Advice],
        0,
        threshold,
    );
    assert_eq!(built.failing_count(), failing);
    assert_eq!(built.is_failure(), is_failure);
    assert_eq!(built.threshold(), threshold);
}

/// A clean report must not be a failure whatever the threshold.
#[test]
fn a_clean_report_never_fails() {
    for threshold in [
        FailOn::Advice,
        FailOn::Warning,
        FailOn::Error,
        FailOn::Never,
    ] {
        assert!(!report!(&[], 0, threshold).is_failure());
    }
}

#[test]
fn the_report_projects_one_diagnostic_per_finding() {
    let built = report!(&[Severity::Error, Severity::Warning], 0, FailOn::Error);
    assert_eq!(built.diagnostics().len(), 2);
}
