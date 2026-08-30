//! Lint findings and their projection onto `miette` diagnostics.
//!
//! A finding is the linter's own record: rule, severity, message, and where it
//! applies. Rendering — human or JSON — goes through the diagnostic projection
//! so that a lint finding and a compiler diagnostic reach a consumer in the
//! same shape.

use std::cmp::Ordering;
use std::sync::Arc;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceCode, SourceSpan};
use thiserror::Error;

use super::document::Span;
use super::rule::RuleMeta;
use super::severity::Severity;

/// Where a finding applies within the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A byte range in the manifest source.
    Span(Span),
    /// A manifest identifier, used when no span could be resolved.
    Symbol(String),
}

/// One reported problem.
#[derive(Debug, Clone)]
pub struct Finding {
    /// The rule that reported it.
    pub meta: &'static RuleMeta,
    /// The severity policy resolved for that rule.
    pub severity: Severity,
    /// What is wrong, phrased for the manifest author.
    pub message: String,
    /// Where it applies.
    pub location: Location,
}

impl Finding {
    /// Build a finding anchored at a source span.
    #[must_use]
    pub fn spanned(
        meta: &'static RuleMeta,
        severity: Severity,
        message: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            meta,
            severity,
            message: message.into(),
            location: Location::Span(span),
        }
    }

    /// Build a finding that names an identifier instead of a span.
    #[must_use]
    pub fn detached(
        meta: &'static RuleMeta,
        severity: Severity,
        message: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            meta,
            severity,
            message: message.into(),
            location: Location::Symbol(symbol.into()),
        }
    }

    /// Report the span this finding is anchored at, when it has one.
    #[must_use]
    pub const fn span(&self) -> Option<Span> {
        match &self.location {
            Location::Span(span) => Some(*span),
            Location::Symbol(_) => None,
        }
    }

    /// Order findings deterministically for output.
    ///
    /// Spanned findings sort by position first so a reader walks the manifest
    /// top to bottom; detached findings follow, ordered by the identifier they
    /// name. Rule name and message break remaining ties, so two runs over the
    /// same manifest emit findings in the same order regardless of the hash
    /// iteration order inside the build graph.
    #[must_use]
    pub const fn sort_key(&self) -> (usize, usize, &str, &str, &str) {
        let (rank, offset, symbol) = match &self.location {
            Location::Span(span) => (0, span.start, ""),
            Location::Symbol(symbol) => (1, 0, symbol.as_str()),
        };
        (rank, offset, symbol, self.meta.name, self.message.as_str())
    }

    /// Compare two findings by their output order.
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }

    /// Project this finding onto a diagnostic carrying the manifest source.
    #[must_use]
    pub fn to_diagnostic(&self, source: &Arc<NamedSource<String>>) -> FindingDiagnostic {
        FindingDiagnostic {
            message: self.display_message(),
            code: self.meta.code(),
            severity: self.severity.to_miette(),
            help: self.meta.remediation.to_owned(),
            url: self.meta.doc_url(),
            manifest_source: Arc::clone(source),
            span: self.span().map(SourceSpan::from),
            label: self.meta.name,
        }
    }

    /// Render the message a reader sees, naming the identifier when the
    /// finding has no span to point at.
    #[must_use]
    pub fn display_message(&self) -> String {
        match &self.location {
            Location::Span(_) => self.message.clone(),
            Location::Symbol(symbol) => format!("{symbol}: {}", self.message),
        }
    }
}

/// A lint finding rendered as a `miette` diagnostic.
///
/// The manifest source is shared rather than cloned per finding: a run over a
/// large manifest can report many findings, and each one would otherwise carry
/// its own copy of the whole file.
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
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.to_owned()),
            span,
        ))))
    }
}

#[cfg(test)]
#[path = "finding_tests.rs"]
mod tests;
