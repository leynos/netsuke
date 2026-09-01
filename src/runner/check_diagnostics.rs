//! Miette diagnostic projection for `netsuke check` findings.
//!
//! This adapter owns source-backed diagnostics at the runner boundary. The
//! lint domain returns neutral findings so it can be reused without depending
//! on miette or any particular publication format.

use std::sync::Arc;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceCode, SourceSpan};
use thiserror::Error;

use crate::lint::document::Span;
use crate::lint::{Bounds, Finding, Outcome, Report, Severity};

/// A source-backed lint report ready for command-output adapters.
pub(super) struct CheckReport {
    /// The domain outcome, bounds, and summaries.
    report: Report,
    /// The source diagnostics use to label spanned findings.
    source: Arc<NamedSource<String>>,
}

impl CheckReport {
    /// Combine a neutral lint outcome with the source its diagnostics label.
    #[must_use]
    pub(super) fn new(name: &str, source: String, outcome: Outcome, bounds: Bounds) -> Self {
        Self {
            report: Report::new(outcome, bounds),
            source: Arc::new(NamedSource::new(name, source).with_language("yaml")),
        }
    }

    /// Borrow the neutral report for summaries and result metadata.
    #[must_use]
    pub(super) const fn report(&self) -> &Report {
        &self.report
    }

    /// Project every reported finding onto a source-backed diagnostic.
    #[must_use]
    pub(super) fn diagnostics(&self) -> Vec<FindingDiagnostic> {
        self.report
            .findings()
            .iter()
            .map(|finding| FindingDiagnostic::new(finding, &self.source))
            .collect()
    }
}

/// A lint finding rendered as a `miette` diagnostic.
///
/// The manifest source is shared rather than cloned per finding: a run over a
/// large manifest can report many findings without copying the whole file.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct FindingDiagnostic {
    /// The reader-facing message.
    message: String,
    /// The rule's stable diagnostic code.
    code: String,
    /// The resolved severity.
    severity: miette::Severity,
    /// The rule's remediation, shown as help.
    help: String,
    /// The rule's documentation URL.
    url: String,
    /// The manifest source, shared across every finding of one run.
    manifest_source: Arc<NamedSource<String>>,
    /// The span to label, when the finding has one.
    span: Option<SourceSpan>,
    /// The rule name, used as the span label.
    label: &'static str,
}

impl FindingDiagnostic {
    /// Adapt `finding` into a diagnostic backed by `source`.
    fn new(finding: &Finding, source: &Arc<NamedSource<String>>) -> Self {
        Self {
            message: finding.display_message(),
            code: finding.meta.code(),
            severity: miette_severity(finding.severity),
            help: finding.meta.remediation.to_owned(),
            url: finding.meta.doc_url(),
            manifest_source: Arc::clone(source),
            span: finding.span().map(source_span),
            label: finding.meta.name,
        }
    }
}

/// Convert a neutral byte span into the miette span this adapter renders.
fn source_span(span: Span) -> SourceSpan {
    SourceSpan::new(span.start.into(), span.len())
}

impl Diagnostic for FindingDiagnostic {
    fn code(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new(&self.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(self.severity)
    }

    fn help(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new(&self.help))
    }

    fn url(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new(&self.url))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.span
            .is_some()
            .then(|| self.manifest_source.as_ref() as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let span = self.span?;
        Some(Box::new(std::iter::once(
            LabeledSpan::new_primary_with_span(Some(self.label.to_owned()), span),
        )))
    }
}

/// Map neutral lint severity onto miette's rendering severity.
const fn miette_severity(severity: Severity) -> miette::Severity {
    match severity {
        Severity::Advice => miette::Severity::Advice,
        Severity::Warning => miette::Severity::Warning,
        Severity::Error => miette::Severity::Error,
    }
}

#[cfg(test)]
#[path = "check_diagnostics_tests.rs"]
mod tests;
