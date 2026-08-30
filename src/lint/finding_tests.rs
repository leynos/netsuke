//! Tests for findings and their diagnostic projection.

use std::sync::Arc;

use miette::{Diagnostic, NamedSource};

use super::{Finding, Location};
use crate::lint::document::Span;
use crate::lint::registry;
use crate::lint::severity::Severity;

/// Borrow a registered rule to attach findings to.
fn meta() -> &'static crate::lint::rule::RuleMeta {
    registry::meta_by_name("background-job").expect("the rule should be registered")
}

#[test]
fn a_spanned_finding_reports_its_span_and_message() {
    let finding = Finding::spanned(meta(), Severity::Warning, "detached", Span::new(4, 9));
    assert_eq!(finding.span(), Some(Span::new(4, 9)));
    assert_eq!(finding.display_message(), "detached");
    assert_eq!(finding.location, Location::Span(Span::new(4, 9)));
}

/// A finding without a span names the declaration instead, so a reader can
/// still find it.
#[test]
fn a_detached_finding_names_its_subject() {
    let finding = Finding::detached(meta(), Severity::Advice, "detached", "target `out`");
    assert_eq!(finding.span(), None);
    assert_eq!(finding.display_message(), "target `out`: detached");
}

/// Spanned findings sort by position; detached ones follow, ordered by the
/// name they carry.
#[test]
fn spanned_findings_sort_before_detached_ones() {
    let early = Finding::spanned(meta(), Severity::Warning, "a", Span::new(1, 2));
    let late = Finding::spanned(meta(), Severity::Warning, "a", Span::new(9, 10));
    let detached = Finding::detached(meta(), Severity::Warning, "a", "zzz");
    let mut findings = [detached.clone(), late.clone(), early.clone()];
    findings.sort_by(Finding::compare);
    let spans: Vec<Option<Span>> = findings.iter().map(Finding::span).collect();
    assert_eq!(
        spans,
        vec![early.span(), late.span(), detached.span()],
        "spanned findings should come first, in position order"
    );
}

#[test]
fn the_diagnostic_carries_the_rule_metadata() {
    let source = Arc::new(NamedSource::new("Netsukefile", "abcdefghij".to_owned()));
    let finding = Finding::spanned(meta(), Severity::Warning, "detached", Span::new(2, 5));
    let diagnostic = finding.to_diagnostic(&source);
    assert_eq!(
        diagnostic.code().map(|code| code.to_string()),
        Some(meta().code())
    );
    assert_eq!(diagnostic.severity(), Some(miette::Severity::Warning));
    assert_eq!(
        diagnostic.help().map(|help| help.to_string()),
        Some(meta().remediation.to_owned())
    );
    assert_eq!(
        diagnostic.url().map(|url| url.to_string()),
        Some(meta().doc_url())
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

/// A finding without a span must not claim source code it cannot point into.
#[test]
fn a_detached_diagnostic_carries_no_source() {
    let source = Arc::new(NamedSource::new("Netsukefile", "abc".to_owned()));
    let diagnostic = Finding::detached(meta(), Severity::Advice, "detached", "target `out`")
        .to_diagnostic(&source);
    assert!(diagnostic.source_code().is_none());
    assert!(diagnostic.labels().is_none());
}
