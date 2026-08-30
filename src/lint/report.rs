//! Human and JSON rendering of lint findings.
//!
//! Both branches carry the same per-finding object, because a consumer should
//! parse one finding representation whatever the failure threshold decided.
//! The projection onto `miette` diagnostics is what makes that possible: the
//! result document embeds the same entries the diagnostic document does.

use std::sync::Arc;

use miette::NamedSource;

use super::engine::Outcome;
use super::finding::{Finding, FindingDiagnostic};
use super::severity::{FailOn, Severity};

/// The manifest a report describes.
pub struct NamedManifest<'a> {
    /// The display name diagnostics label the source with.
    pub name: &'a str,
    /// The manifest source text.
    pub source: String,
}

/// How a report is bounded and judged.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    /// Maximum findings to report; zero reports every finding.
    pub limit: usize,
    /// The severity at which findings fail the command.
    pub threshold: FailOn,
}

/// Findings selected for one report, together with what was left out.
pub struct Report {
    /// The manifest source, shared by every rendered finding.
    source: Arc<NamedSource<String>>,
    /// Findings within the reporting limit, in output order.
    reported: Vec<Finding>,
    /// Findings the limit excluded.
    truncated: usize,
    /// Findings a directive silenced.
    suppressed: usize,
    /// The threshold that decides whether the command failed.
    threshold: FailOn,
}

impl Report {
    /// Build a report from an outcome, bounding it to `limit` findings.
    ///
    /// A `limit` of zero reports every finding. Truncation drops the least
    /// severe findings last, so raising the limit only ever adds entries to
    /// the end of the list.
    #[must_use]
    pub fn new(manifest: NamedManifest<'_>, outcome: Outcome, bounds: Bounds) -> Self {
        let total = outcome.findings.len();
        let mut reported = outcome.findings;
        if bounds.limit > 0 && total > bounds.limit {
            reported.truncate(bounds.limit);
        }
        let truncated = total.saturating_sub(reported.len());
        Self {
            source: Arc::new(
                NamedSource::new(manifest.name, manifest.source).with_language("yaml"),
            ),
            reported,
            truncated,
            suppressed: outcome.suppressed,
            threshold: bounds.threshold,
        }
    }

    /// Borrow the reported findings.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.reported
    }

    /// Report how many findings the limit excluded.
    #[must_use]
    pub const fn truncated(&self) -> usize {
        self.truncated
    }

    /// Report how many findings a directive silenced.
    #[must_use]
    pub const fn suppressed(&self) -> usize {
        self.suppressed
    }

    /// Count the reported findings at `severity`.
    #[must_use]
    pub fn count_at(&self, severity: Severity) -> usize {
        self.reported
            .iter()
            .filter(|finding| finding.severity == severity)
            .count()
    }

    /// Count the reported findings that reach the failure threshold.
    #[must_use]
    pub fn failing_count(&self) -> usize {
        self.reported
            .iter()
            .filter(|finding| self.threshold.is_reached_by(finding.severity))
            .count()
    }

    /// Report whether the command fails because of these findings.
    #[must_use]
    pub fn is_failure(&self) -> bool {
        self.failing_count() > 0
    }

    /// Report the threshold this report was built against.
    #[must_use]
    pub const fn threshold(&self) -> FailOn {
        self.threshold
    }

    /// Project every reported finding onto a diagnostic.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<FindingDiagnostic> {
        self.reported
            .iter()
            .map(|finding| finding.to_diagnostic(&self.source))
            .collect()
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
