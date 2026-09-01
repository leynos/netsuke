//! Lint findings and their neutral presentation data.
//!
//! A finding is the linter's own record: rule, severity, message, and where it
//! applies. Runner adapters project findings onto their chosen output format,
//! keeping the lint domain independent from diagnostic frameworks.

use std::cmp::Ordering;

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
    /// Build a finding at an already-resolved location.
    ///
    /// The public constructors differ only in how they describe where the
    /// finding applies, so they share the field initialization and each is
    /// left saying only which [`Location`] it builds.
    fn new(
        meta: &'static RuleMeta,
        severity: Severity,
        message: impl Into<String>,
        location: Location,
    ) -> Self {
        Self {
            meta,
            severity,
            message: message.into(),
            location,
        }
    }

    /// Build a finding anchored at a source span.
    #[must_use]
    pub fn spanned(
        meta: &'static RuleMeta,
        severity: Severity,
        message: impl Into<String>,
        span: Span,
    ) -> Self {
        Self::new(meta, severity, message, Location::Span(span))
    }

    /// Build a finding that names an identifier instead of a span.
    #[must_use]
    pub fn detached(
        meta: &'static RuleMeta,
        severity: Severity,
        message: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self::new(meta, severity, message, Location::Symbol(symbol.into()))
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

#[cfg(test)]
#[path = "finding_tests.rs"]
mod tests;
