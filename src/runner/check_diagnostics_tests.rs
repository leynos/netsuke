//! Tests for the runner-owned lint diagnostic adapter.

use miette::Diagnostic;

use super::super::check_documentation::rule_documentation_url;
use super::{CheckReport, FindingDiagnostic};
use crate::lint::document::Span;
use crate::lint::registry;
use crate::lint::{Bounds, FailOn, Finding, Outcome, Severity};

/// Borrow a registered rule to attach findings to each test case.
macro_rules! meta {
    () => {
        registry::meta_by_name("background-job").expect("the rule should be registered")
    };
}

/// Build one source-backed report for the supplied findings.
fn report(findings: Vec<Finding>) -> CheckReport {
    CheckReport::new(
        "Netsukefile",
        "abcdefghij".to_owned(),
        Outcome {
            findings,
            suppressed: 0,
        },
        Bounds {
            limit: 0,
            threshold: FailOn::Error,
        },
    )
}

/// Preserve rule metadata and source labels in the miette adapter.
#[test]
fn a_spanned_finding_carries_the_rule_metadata() {
    let meta = meta!();
    let report = report(vec![Finding::spanned(
        meta,
        Severity::Warning,
        "detached",
        Span::new(2, 5),
    )]);
    let diagnostics = report.diagnostics();
    let diagnostic: &FindingDiagnostic = diagnostics
        .first()
        .expect("the report should produce one diagnostic");
    assert_eq!(
        diagnostic.code().map(|code| code.to_string()),
        Some(meta.code())
    );
    assert_eq!(diagnostic.severity(), Some(miette::Severity::Warning));
    assert_eq!(
        diagnostic.help().map(|help| help.to_string()),
        Some(meta.remediation.to_owned())
    );
    assert_eq!(
        diagnostic.url().map(|url| url.to_string()),
        Some(rule_documentation_url(meta.name))
    );
    assert!(diagnostic.source_code().is_some());
    let labels: Vec<_> = diagnostic
        .labels()
        .map(Iterator::collect::<Vec<_>>)
        .unwrap_or_default();
    let label = labels
        .first()
        .expect("the diagnostic should carry one label");
    assert_eq!(labels.len(), 1);
    assert!(label.primary(), "the span should be the primary label");
}

/// Avoid claiming source code for a finding without a resolved span.
#[test]
fn a_detached_finding_carries_no_source() {
    let report = report(vec![Finding::detached(
        meta!(),
        Severity::Advice,
        "detached",
        "target `out`",
    )]);
    let diagnostics = report.diagnostics();
    let diagnostic = diagnostics
        .first()
        .expect("the report should produce one diagnostic");
    assert!(diagnostic.source_code().is_none());
    assert!(diagnostic.labels().is_none());
}
