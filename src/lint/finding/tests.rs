//! Tests for neutral lint findings.

use super::{Finding, Location};
use crate::lint::document::Span;
use crate::lint::registry;
use crate::lint::severity::Severity;

/// Borrow a registered rule to attach findings to.
macro_rules! meta {
    () => {
        registry::meta_by_name("background-job").expect("the rule should be registered")
    };
}

#[test]
fn a_spanned_finding_reports_its_span_and_message() {
    let finding = Finding::spanned(meta!(), Severity::Warning, "detached", Span::new(4, 9));
    assert_eq!(finding.span(), Some(Span::new(4, 9)));
    assert_eq!(finding.display_message(), "detached");
    assert_eq!(finding.location, Location::Span(Span::new(4, 9)));
}

/// A finding without a span names the declaration instead, so a reader can
/// still find it.
#[test]
fn a_detached_finding_names_its_subject() {
    let finding = Finding::detached(meta!(), Severity::Advice, "detached", "target `out`");
    assert_eq!(finding.span(), None);
    assert_eq!(finding.display_message(), "target `out`: detached");
}

/// Spanned findings sort by position; detached ones follow, ordered by the
/// name they carry.
#[test]
fn spanned_findings_sort_before_detached_ones() {
    let early = Finding::spanned(meta!(), Severity::Warning, "a", Span::new(1, 2));
    let late = Finding::spanned(meta!(), Severity::Warning, "a", Span::new(9, 10));
    let detached = Finding::detached(meta!(), Severity::Warning, "a", "zzz");
    let mut findings = [detached.clone(), late.clone(), early.clone()];
    findings.sort_by(Finding::compare);
    let spans: Vec<Option<Span>> = findings.iter().map(Finding::span).collect();
    assert_eq!(
        spans,
        vec![early.span(), late.span(), detached.span()],
        "spanned findings should come first, in position order"
    );
}
